// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Manages chain synchronisation process.
//! - tries to download most recent header from the other peers
//! - also supplies downloaded data to other peers

use std::cmp;
use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime};

use failure::Error;
use itertools::Itertools;
use riker::actors::*;
use slog::{debug, info, trace, warn};

use crypto::hash::{BlockHash, ChainId, HashType, OperationHash};
use networking::p2p::network_channel::{NetworkChannelMsg, NetworkChannelRef, PeerBootstrapped};
use networking::p2p::peer::{PeerRef, SendMessage};
use storage::{BlockHeaderWithHash, BlockMetaStorage, BlockStorage, BlockStorageReader, MempoolStorage, OperationsStorage, OperationsStorageReader, StorageError};
use storage::block_meta_storage::BlockMetaStorageReader;
use storage::mempool_storage::MempoolOperationType;
use storage::persistent::PersistentStorage;
use tezos_messages::p2p::binary_message::MessageHash;
use tezos_messages::p2p::encoding::prelude::*;

use crate::Head;
use crate::shell_channel::{AllBlockOperationsReceived, BlockReceived, CurrentMempoolState, MempoolOperationReceived, ShellChannelMsg, ShellChannelRef, ShellChannelTopic};
use crate::state::block_state::{BlockchainState, MissingBlock};
use crate::state::operations_state::{MissingOperations, OperationsState};
use crate::subscription::*;

/// Limit to how many blocks to request in a batch
const BLOCK_HEADERS_BATCH_SIZE: usize = 10;
/// Limit to how many block operations to request in a batch
const BLOCK_OPERATIONS_BATCH_SIZE: usize = 10;
/// Limit to how many mempool operations to request in a batch
const MEMPOOL_OPERATIONS_BATCH_SIZE: usize = 10;
/// How often to check chain completeness
const CHECK_CHAIN_COMPLETENESS_INTERVAL: Duration = Duration::from_secs(30);
/// How often to ask all connected peers for current branch
const ASK_CURRENT_BRANCH_INTERVAL: Duration = Duration::from_secs(15);
/// How often to print stats in logs
const LOG_INTERVAL: Duration = Duration::from_secs(60);
/// After this time we will disconnect peer if his current head level stays the same
const CURRENT_HEAD_LEVEL_UPDATE_TIMEOUT: Duration = Duration::from_secs(120);
/// After this time peer will be disconnected if it fails to respond to our request
const SILENT_PEER_TIMEOUT: Duration = Duration::from_secs(30);
/// Maximum timeout duration in sandbox mode (do not disconnect peers in sandbox mode)
const SILENT_PEER_TIMEOUT_SANDBOX: Duration = Duration::from_secs(31_536_000);
/// After this interval we will rehydrate state if no new blocks are applied
const STALLED_CHAIN_COMPLETENESS_TIMEOUT: Duration = Duration::from_secs(240);
const BLOCK_HASH_ENCODING: HashType = HashType::BlockHash;
/// Mempool operation time to live
const MEMPOOL_OPERATION_TTL: Duration = Duration::from_secs(60);

/// Message commands [`ChainManager`] to disconnect stalled peers.
#[derive(Clone, Debug)]
pub struct DisconnectStalledPeers;

/// Message commands [`ChainManager`] to check completeness of the chain.
#[derive(Clone, Debug)]
pub struct CheckChainCompleteness;

/// Message commands [`ChainManager`] to check if all mempool operations were fetched from peer.
#[derive(Clone, Debug)]
pub struct CheckMempoolCompleteness;

/// Message commands [`ChainManager`] to ask all connected peers for their current head.
#[derive(Clone, Debug)]
pub struct AskPeersAboutCurrentBranch;

/// Message commands [`ChainManager`] to log its internal stats.
#[derive(Clone, Debug)]
pub struct LogStats;

/// This struct holds info about local and remote "current" head
#[derive(Clone, Debug)]
struct CurrentHead {
    /// Represents local current head. Value here is the same as the
    /// hash of the last applied block.
    local: Option<Head>,
    /// Remote current head. This represents info about
    /// the current branch with the highest level received from network.
    remote: Option<Head>,
}

impl CurrentHead {
    fn need_update_remote_level(&self, new_remote_level: i32) -> bool {
        match &self.remote {
            None => true,
            Some(current_remote_head) => new_remote_level > current_remote_head.level
        }
    }

    fn local_debug_info(&self) -> (String, i32) {
        match &self.local {
            None => ("-none-".to_string(), 0 as i32),
            Some(head) => head.to_debug_info()
        }
    }

    fn remote_debug_info(&self) -> (String, i32) {
        match &self.remote {
            None => ("-none-".to_string(), 0 as i32),
            Some(head) => head.to_debug_info()
        }
    }
}

/// Holds various stats with info about internal synchronization.
struct Stats {
    /// Count of received blocks
    unseen_block_count: usize,
    /// Lest time when previously not seen block was received
    unseen_block_last: Instant,
    /// Last time when previously unseen operations were received
    unseen_block_operations_last: Instant,
    /// ID of the last applied block
    applied_block_level: Option<i32>,
    /// Last time a block was applied
    applied_block_last: Option<Instant>,
    /// Last time state was hydrated
    hydrated_state_last: Option<Instant>,
}

/// Purpose of this actor is to perform chain synchronization.
#[actor(DisconnectStalledPeers, CheckChainCompleteness, CheckMempoolCompleteness, AskPeersAboutCurrentBranch, LogStats, NetworkChannelMsg, ShellChannelMsg, SystemEvent, DeadLetter)]
pub struct ChainManager {
    /// All events generated by the network layer will end up in this channel
    network_channel: NetworkChannelRef,
    /// All events from shell will be published to this channel
    shell_channel: ShellChannelRef,
    /// Holds the state of all peers
    peers: HashMap<ActorUri, PeerState>,
    /// Block storage
    block_storage: Box<dyn BlockStorageReader>,
    /// Block meta storage
    block_meta_storage: Box<dyn BlockMetaStorageReader>,
    /// Operations storage
    operations_storage: Box<dyn OperationsStorageReader>,
    /// Mempool operation storage
    mempool_storage: MempoolStorage,
    /// Holds state of the blockchain
    chain_state: BlockchainState,
    /// Holds state of the operations
    operations_state: OperationsState,
    /// Current head information
    current_head: CurrentHead,
    // current last known mempool state
    current_mempool_state: Option<CurrentMempoolState>,
    /// Internal stats
    stats: Stats,
    /// Indicates that system is shutting down
    shutting_down: bool,
    /// Indicates node mode
    is_sandbox: bool,
}

/// Reference to [chain manager](ChainManager) actor.
pub type ChainManagerRef = ActorRef<ChainManagerMsg>;

impl ChainManager {
    /// Create new actor instance.
    pub fn actor(sys: &impl ActorRefFactory, network_channel: NetworkChannelRef, shell_channel: ShellChannelRef, persistent_storage: &PersistentStorage, chain_id: &ChainId, is_sandbox: bool) -> Result<ChainManagerRef, CreateError> {
        sys.actor_of_props::<ChainManager>(
            ChainManager::name(),
            Props::new_args((network_channel, shell_channel, persistent_storage.clone(), chain_id.clone(), is_sandbox)),
        )
    }

    /// The `ChainManager` is intended to serve as a singleton actor so that's why
    /// we won't support multiple names per instance.
    fn name() -> &'static str {
        "chain-manager"
    }

    fn check_mempool_completeness(&mut self, _ctx: &Context<ChainManagerMsg>) {
        let ChainManager { peers, .. } = self;

        // check for missing mempool operations
        peers.values_mut()
            .filter(|peer| !peer.missing_mempool_operations.is_empty())
            .filter(|peer| peer.available_block_operations_queue_capacity() > 0)
            .for_each(|peer| {
                let num_opts_to_get = cmp::min(peer.missing_mempool_operations.len(), peer.available_mempool_operations_queue_capacity());
                let ops_to_enqueue = peer.missing_mempool_operations
                    .drain(0..num_opts_to_get)
                    .collect::<Vec<_>>();

                let ttl = SystemTime::now() + MEMPOOL_OPERATION_TTL;
                ops_to_enqueue.iter().cloned()
                    .for_each(|(op_hash, op_type)| {
                        peer.queued_mempool_operations.insert(op_hash, (op_type, ttl));
                    });

                let ops_to_get = ops_to_enqueue.into_iter()
                    .map(|(op_hash, _)| op_hash)
                    .collect();

                peer.mempool_operations_request_last = Instant::now();
                tell_peer(GetOperationsMessage::new(ops_to_get).into(), peer);
            });
    }

    /// Check for missing blocks in local chain copy, and schedule downloading for those blocks
    fn check_chain_completeness(&mut self, ctx: &Context<ChainManagerMsg>) -> Result<(), Error> {
        let ChainManager { peers, chain_state, operations_state, stats, .. } = self;

        // check for missing blocks
        if chain_state.has_missing_blocks() {
            peers.values_mut()
                .filter(|peer| peer.current_head_level.is_some())
                .filter(|peer| peer.available_block_queue_capacity() > 0)
                .sorted_by_key(|peer| peer.available_block_queue_capacity()).rev()
                .for_each(|peer| {
                    let mut missing_blocks = chain_state.drain_missing_blocks(peer.available_block_queue_capacity(), peer.current_head_level.unwrap());
                    if !missing_blocks.is_empty() {
                        let queued_blocks = missing_blocks.drain(..)
                            .map(|missing_block| {
                                let missing_block_hash = missing_block.block_hash.clone();
                                if peer.queued_block_headers.insert(missing_block_hash.clone(), missing_block).is_none() {
                                    // block was not already present in queue
                                    Some(missing_block_hash)
                                } else {
                                    // block was already in queue
                                    None
                                }
                            })
                            .filter_map(|missing_block_hash| missing_block_hash)
                            .collect::<Vec<_>>();

                        if !queued_blocks.is_empty() {
                            peer.block_request_last = Instant::now();
                            tell_peer(GetBlockHeadersMessage::new(queued_blocks).into(), peer);
                        }
                    }
                });
        }

        // check for missing block operations
        if operations_state.has_missing_block_operations() {
            peers.values_mut()
                .filter(|peer| peer.current_head_level.is_some())
                .filter(|peer| peer.available_block_operations_queue_capacity() > 0)
                .sorted_by_key(|peer| peer.available_block_operations_queue_capacity()).rev()
                .for_each(|peer| {
                    let missing_operations = operations_state.drain_missing_block_operations(peer.available_block_operations_queue_capacity(), peer.current_head_level.unwrap());
                    if !missing_operations.is_empty() {
                        let queued_operations = missing_operations.iter()
                            .map(|missing_operation| {
                                if peer.queued_block_operations.insert(missing_operation.block_hash.clone(), missing_operation.clone()).is_none() {
                                    // operations were not already present in queue
                                    Some(missing_operation)
                                } else {
                                    // operations were already in queue
                                    None
                                }
                            })
                            .filter_map(|missing_operation| missing_operation)
                            .collect::<Vec<_>>();

                        if !queued_operations.is_empty() {
                            peer.block_operations_request_last = Instant::now();
                            queued_operations.iter()
                                .for_each(|&missing_operation| tell_peer(GetOperationsForBlocksMessage::new(missing_operation.into()).into(), peer));
                        }
                    }
                });
        }

        if let (Some(applied_block_last), Some(hydrated_state_last)) = (stats.applied_block_last, stats.hydrated_state_last) {
            if (applied_block_last.elapsed() > STALLED_CHAIN_COMPLETENESS_TIMEOUT) && (hydrated_state_last.elapsed() > STALLED_CHAIN_COMPLETENESS_TIMEOUT) {
                self.hydrate_state(ctx);
            }
        }

        Ok(())
    }

    fn process_network_channel_message(&mut self, ctx: &Context<ChainManagerMsg>, msg: NetworkChannelMsg) -> Result<(), Error> {
        let ChainManager {
            peers,
            chain_state,
            operations_state,
            shell_channel,
            block_storage,
            operations_storage,
            stats,
            mempool_storage,
            current_head,
            ..
        } = self;

        match msg {
            NetworkChannelMsg::PeerBootstrapped(PeerBootstrapped::Success { peer, peer_metadata, .. }) => {
                let log = ctx.system.log().new(slog::o!("peer" => peer.name().to_string()));

                debug!(log, "Requesting current branch");
                let peer = PeerState::new(peer, peer_metadata);
                // store peer
                let actor_uri = peer.peer_ref.uri().clone();
                self.peers.insert(actor_uri.clone(), peer);
                // retrieve mutable reference and use it as `tell_peer()` parameter
                let peer = self.peers.get_mut(&actor_uri).unwrap();
                tell_peer(GetCurrentBranchMessage::new(chain_state.get_chain_id().clone()).into(), peer);
            }
            NetworkChannelMsg::PeerMessageReceived(received) => {
                let log = ctx.system.log().new(slog::o!("peer" => received.peer.name().to_string()));

                match peers.get_mut(received.peer.uri()) {
                    Some(peer) => {
                        for message in received.message.messages() {
                            match message {
                                PeerMessage::CurrentBranch(message) => {
                                    debug!(log, "Received current branch");
                                    if message.current_branch().current_head().level() > 0 {
                                        // schedule predecessor
                                        chain_state.push_missing_block(
                                            MissingBlock::with_level_guess(
                                                message.current_branch().current_head().predecessor().clone(),
                                                message.current_branch().current_head().level() - 1,
                                            )
                                        )?;

                                        // schedule current_head
                                        chain_state.push_missing_block(
                                            MissingBlock::with_level(
                                                message.current_branch().current_head().message_hash()?,
                                                message.current_branch().current_head().level(),
                                            )
                                        )?
                                    }

                                    // schedule history - we try to prioritize download from the beginning, so the history is reversed here
                                    chain_state.push_missing_history(
                                        message.current_branch().history().iter().cloned().rev().collect(),
                                        message.current_branch().current_head().level(),
                                    )?;

                                    let peer_current_head = message.current_branch().current_head();

                                    // if needed, update remote current head
                                    if current_head.need_update_remote_level(message.current_branch().current_head().level()) {
                                        current_head.remote = Some(Head {
                                            hash: message.current_branch().current_head().message_hash()?,
                                            level: message.current_branch().current_head().level(),
                                        });
                                    }

                                    // update peer stats
                                    if peer.current_head_level.is_none() || (message.current_branch().current_head().level() > peer.current_head_level.unwrap()) {
                                        peer.current_head_level = Some(message.current_branch().current_head().level());
                                        peer.current_head_update_last = Instant::now();
                                    }

                                    // notify others that new block was received
                                    shell_channel.tell(
                                        Publish {
                                            msg: BlockReceived {
                                                hash: peer_current_head.message_hash()?,
                                                level: peer_current_head.level(),
                                            }.into(),
                                            topic: ShellChannelTopic::ShellEvents.into(),
                                        }, Some(ctx.myself().into()));

                                    // trigger CheckChainCompleteness
                                    ctx.myself().tell(CheckChainCompleteness, None);
                                }
                                PeerMessage::GetCurrentBranch(message) => {
                                    debug!(log, "Current branch requested by a peer");
                                    if chain_state.get_chain_id() == &message.chain_id {
                                        if let Some(current_head_local) = &current_head.local {
                                            if let Some(current_head) = block_storage.get(&current_head_local.hash)? {
                                                let history = chain_state.get_history()?;
                                                let msg = CurrentBranchMessage::new(chain_state.get_chain_id().clone(), CurrentBranch::new((*current_head.header).clone(), history));
                                                tell_peer(msg.into(), peer);
                                            }
                                        }
                                    }
                                }
                                PeerMessage::BlockHeader(message) => {
                                    let block_header_with_hash = BlockHeaderWithHash::new(message.block_header().clone()).unwrap();
                                    match peer.queued_block_headers.remove(&block_header_with_hash.hash) {
                                        Some(_) => {
                                            trace!(log, "Received block header");
                                            peer.block_response_last = Instant::now();

                                            let is_new_block =
                                                chain_state.process_block_header(&block_header_with_hash, &log)
                                                    .and(operations_state.process_block_header(&block_header_with_hash))?;

                                            if is_new_block {
                                                // update stats
                                                stats.unseen_block_last = Instant::now();
                                                stats.unseen_block_count += 1;

                                                // trigger CheckChainCompleteness
                                                ctx.myself().tell(CheckChainCompleteness, None);

                                                // notify others that new block was received
                                                shell_channel.tell(
                                                    Publish {
                                                        msg: BlockReceived {
                                                            hash: block_header_with_hash.hash,
                                                            level: block_header_with_hash.header.level(),
                                                        }.into(),
                                                        topic: ShellChannelTopic::ShellEvents.into(),
                                                    }, Some(ctx.myself().into()));
                                            }
                                        }
                                        None => {
                                            warn!(log, "Received unexpected block header"; "block_header_hash" => BLOCK_HASH_ENCODING.bytes_to_string(&block_header_with_hash.hash));
                                        }
                                    }
                                }
                                PeerMessage::GetBlockHeaders(message) => {
                                    for block_hash in message.get_block_headers() {
                                        if let Some(block) = block_storage.get(block_hash)? {
                                            let msg: BlockHeaderMessage = (*block.header).clone().into();
                                            tell_peer(msg.into(), peer);
                                        }
                                    }
                                }
                                PeerMessage::GetCurrentHead(message) => {
                                    debug!(log, "Current head requested");
                                    if chain_state.get_chain_id() == message.chain_id() {
                                        if let Some(current_head_local) = &current_head.local {
                                            if let Some(current_head) = block_storage.get(&current_head_local.hash)? {
                                                let msg = CurrentHeadMessage::new(
                                                    chain_state.get_chain_id().clone(),
                                                    (*current_head.header).clone(),
                                                    resolve_mempool_to_send_to_peer(&peer, &self.current_mempool_state, &current_head_local),
                                                );
                                                tell_peer(msg.into(), peer);
                                            }
                                        }
                                    }
                                }
                                PeerMessage::OperationsForBlocks(operations) => {
                                    let block_hash = operations.operations_for_block().hash().clone();
                                    match peer.queued_block_operations.get_mut(&block_hash) {
                                        Some(missing_operations) => {
                                            let operation_was_expected = missing_operations.validation_passes.remove(&operations.operations_for_block().validation_pass());
                                            if operation_was_expected {
                                                peer.block_operations_response_last = Instant::now();
                                                trace!(log, "Received operations validation pass"; "validation_pass" => operations.operations_for_block().validation_pass(), "block_header_hash" => BLOCK_HASH_ENCODING.bytes_to_string(&block_hash));

                                                if operations_state.process_block_operations(&operations)? {
                                                    // update stats
                                                    stats.unseen_block_operations_last = Instant::now();

                                                    // trigger CheckChainCompleteness
                                                    ctx.myself().tell(CheckChainCompleteness, None);

                                                    // notify others that new all operations for block were received
                                                    let block = block_storage.get(&block_hash)?.ok_or(StorageError::MissingKey)?;
                                                    shell_channel.tell(
                                                        Publish {
                                                            msg: AllBlockOperationsReceived {
                                                                hash: block.hash,
                                                                level: block.header.level(),
                                                            }.into(),
                                                            topic: ShellChannelTopic::ShellEvents.into(),
                                                        }, Some(ctx.myself().into()));

                                                    // remove operations from queue
                                                    peer.queued_block_operations.remove(&block_hash);
                                                }
                                            } else {
                                                warn!(log, "Received unexpected validation pass"; "validation_pass" => operations.operations_for_block().validation_pass(), "block_header_hash" => BLOCK_HASH_ENCODING.bytes_to_string(&block_hash));
                                                ctx.system.stop(received.peer.clone());
                                            }
                                        }
                                        None => {
                                            warn!(log, "Received unexpected operations");
                                            ctx.system.stop(received.peer.clone());
                                        }
                                    }
                                }
                                PeerMessage::GetOperationsForBlocks(message) => {
                                    for get_op in message.get_operations_for_blocks() {
                                        if get_op.validation_pass() < 0 {
                                            continue;
                                        }

                                        let key = get_op.into();
                                        if let Some(op) = operations_storage.get(&key)? {
                                            tell_peer(op.into(), peer);
                                        }
                                    }
                                }
                                PeerMessage::CurrentHead(message) => {
                                    debug!(log, "Current head received");
                                    if chain_state.get_chain_id() == message.chain_id() {
                                        let peer_current_mempool = message.current_mempool();

                                        // all operations (known_valid + pending) should be added to pending and validated afterwards
                                        // enqueue mempool operations for retrieval
                                        peer_current_mempool.known_valid().iter().cloned()
                                            .for_each(|operation_hash| {
                                                peer.missing_mempool_operations.push((operation_hash, MempoolOperationType::Pending));
                                            });
                                        peer_current_mempool.pending().iter().cloned()
                                            .for_each(|operation_hash| {
                                                peer.missing_mempool_operations.push((operation_hash, MempoolOperationType::Pending));
                                            });

                                        // trigger CheckMempoolCompleteness
                                        ctx.myself().tell(CheckMempoolCompleteness, None);
                                    }
                                }
                                PeerMessage::GetOperations(message) => {
                                    debug!(log, "Get operations received (mempool)");
                                    let requested_operations: &Vec<OperationHash> = message.get_operations();
                                    for operation_hash in requested_operations {
                                        // TODO: where to look for operations for advertised mempool?
                                        // TODO: if not found here, check regular operation storage?
                                        if let Some(found) = mempool_storage.find(&operation_hash)? {
                                            tell_peer(found.into(), peer);
                                        }
                                    }
                                }
                                PeerMessage::Operation(message) => {
                                    debug!(log, "Received mempool message");
                                    match peer.queued_mempool_operations.remove(&message.operation().message_hash()?) {
                                        Some((op_type, op_ttl)) => {
                                            // store mempool operation
                                            peer.mempool_operations_response_last = Instant::now();
                                            mempool_storage.put(op_type.clone(), message.clone(), op_ttl)?;

                                            // trigger CheckMempoolCompleteness
                                            ctx.myself().tell(CheckMempoolCompleteness, None);

                                            // notify others that new operation was received
                                            shell_channel.tell(
                                                Publish {
                                                    msg: MempoolOperationReceived {
                                                        operation_hash: message.operation().message_hash()?.clone(),
                                                        operation_type: op_type.clone(),
                                                    }.into(),
                                                    topic: ShellChannelTopic::ShellEvents.into(),
                                                }, Some(ctx.myself().into()));
                                        }
                                        None => debug!(log, "Unexpected mempool operation received")
                                    }
                                }
                                PeerMessage::Bootstrap => {
                                    // on bootstrap reset peer state
                                }
                                ignored_message => trace!(log, "Ignored message"; "message" => format!("{:?}", ignored_message))
                            }
                        }
                    }
                    None => debug!(log, "Received message from non-existing peer")
                }
            }
            _ => (),
        }

        Ok(())
    }

    fn process_shell_channel_message(&mut self, ctx: &Context<ChainManagerMsg>, msg: ShellChannelMsg) -> Result<(), Error> {
        match msg {
            ShellChannelMsg::BlockApplied(message) => {
                self.current_head.local = Some(Head {
                    hash: message.header().hash.clone(),
                    level: message.header().header.level(),
                });
                self.stats.applied_block_level = Some(message.header().header.level());
                self.stats.applied_block_last = Some(Instant::now());
            }
            ShellChannelMsg::MempoolStateChanged(new_mempool_state) => {
                // prepare mempool/header to send to peers
                let (mempool_to_send, header_to_send) = match &new_mempool_state.head {
                    Some(head) => {
                        if let Some(header) = self.block_storage.get(&head.hash)? {
                            (resolve_mempool_to_send(&new_mempool_state), Some((*header.header).clone()))
                        } else {
                            (Mempool::default(), None)
                        }
                    }
                    None => (Mempool::default(), None)
                };

                // set current mempool state
                self.current_mempool_state = Some(new_mempool_state);

                // send CurrentHead, only if we have anything in mempool (just to peers with enabled mempool)
                if let Some(header_to_send) = header_to_send {
                    if !mempool_to_send.is_empty() {
                        let ChainManager { peers, chain_state, .. } = self;
                        peers.iter_mut()
                            .filter(|(_, peer)| peer.mempool_enabled)
                            .for_each(|(_, peer)| {
                                tell_peer(
                                    CurrentHeadMessage::new(
                                        chain_state.get_chain_id().clone(),
                                        header_to_send.clone(),
                                        mempool_to_send.clone(),
                                    ).into(),
                                    peer,
                                )
                            });
                    }
                }
            }
            ShellChannelMsg::InjectBlock(inject_data) => {
                let level = inject_data.block_header.level();
                let block_header_with_hash = BlockHeaderWithHash::new(inject_data.block_header).unwrap();
                let log = ctx.system.log().new(slog::o!("injection" => "block".to_string()));

                // this should  allways return true, as we are injecting a forged new block
                let is_new_block =
                    self.chain_state.process_block_header(&block_header_with_hash, &log)
                        .and(self.operations_state.process_injected_block_header(&block_header_with_hash))?;

                if is_new_block {
                    // update stats
                    self.stats.unseen_block_last = Instant::now();
                    self.stats.unseen_block_count += 1;

                    // trigger CheckChainCompleteness
                    ctx.myself().tell(CheckChainCompleteness, None);

                    // notify others that new block (header) was received
                    self.shell_channel.tell(
                        Publish {
                            msg: BlockReceived {
                                hash: block_header_with_hash.hash,
                                level: block_header_with_hash.header.level(),
                            }.into(),
                            topic: ShellChannelTopic::ShellEvents.into(),
                        }, Some(ctx.myself().into()));

                    // handle operations
                    // Note: When injecting the first block with the protocol activation, there are no validation passes (therefore no operations)
                    // so we handle operations only above level 1 
                    if level > 1 {
                        // safe unwrap after above first block
                        let operations = inject_data.operations.clone().unwrap();
                        let header = block_header_with_hash.header;
                        let op_paths = inject_data.operation_paths.clone().unwrap();

                        // iterate through all validation passes 
                        for (idx, ops) in operations.iter().enumerate() {
                            let opb = OperationsForBlock::new(header.message_hash()?, idx as i8);

                            // create OperationsForBlocksMessage - the operations are stored in DB as a OperationsForBlocksMessage per validation pass per block
                            // e.g one block -> 4 validation passes -> 4 OperationsForBlocksMessage to store for the block
                            let msg: OperationsForBlocksMessage = OperationsForBlocksMessage::new(opb, op_paths.get(idx).unwrap().to_owned(), ops.clone());
                            if self.operations_state.process_block_operations(&msg)? {
                                // update stats
                                self.stats.unseen_block_operations_last = Instant::now();

                                // trigger CheckChainCompleteness
                                ctx.myself().tell(CheckChainCompleteness, None);

                                // notify others that new all operations for block were received
                                let block = self.block_storage.get(&header.message_hash()?)?.ok_or(StorageError::MissingKey)?;
                                self.shell_channel.tell(
                                    Publish {
                                        msg: AllBlockOperationsReceived {
                                            hash: block.hash,
                                            level: block.header.level(),
                                        }.into(),
                                        topic: ShellChannelTopic::ShellEvents.into(),
                                    }, Some(ctx.myself().into()));
                            }
                        }
                    }
                }
            }
            ShellChannelMsg::ShuttingDown(_) => {
                self.shutting_down = true;
                unsubscribe_from_dead_letters(ctx.system.dead_letters(), ctx.myself());
            }
            _ => ()
        }

        Ok(())
    }

    fn hydrate_state(&mut self, ctx: &Context<ChainManagerMsg>) {
        info!(ctx.system.log(), "Hydrating block state");
        self.chain_state.hydrate().expect("Failed to hydrate chain state");

        info!(ctx.system.log(), "Hydrating operations state");
        self.operations_state.hydrate().expect("Failed to hydrate operations state");

        info!(ctx.system.log(), "Loading current head");
        self.current_head.local = self.block_meta_storage.load_current_head().expect("Failed to load current head")
            .map(|(hash, level)| Head { hash, level });

        let (local_head, local_head_level) = self.current_head.local_debug_info();
        info!(
            ctx.system.log(),
            "Hydrating completed successfully";
            "local_head" => local_head,
            "local_head_level" => local_head_level,
            "missing_blocks" => self.chain_state.missing_blocks_count(),
            "missing_block_operations" => self.operations_state.missing_block_operations_count(),
        );
        self.stats.hydrated_state_last = Some(Instant::now());
    }
}

impl ActorFactoryArgs<(NetworkChannelRef, ShellChannelRef, PersistentStorage, ChainId, bool)> for ChainManager {
    fn create_args((network_channel, shell_channel, persistent_storage, chain_id, is_sandbox): (NetworkChannelRef, ShellChannelRef, PersistentStorage, ChainId, bool)) -> Self {
        ChainManager {
            network_channel,
            shell_channel,
            block_storage: Box::new(BlockStorage::new(&persistent_storage)),
            block_meta_storage: Box::new(BlockMetaStorage::new(&persistent_storage)),
            operations_storage: Box::new(OperationsStorage::new(&persistent_storage)),
            mempool_storage: MempoolStorage::new(&persistent_storage),
            chain_state: BlockchainState::new(&persistent_storage, &chain_id),
            operations_state: OperationsState::new(&persistent_storage, &chain_id),
            peers: HashMap::new(),
            current_head: CurrentHead {
                local: None,
                remote: None,
            },
            current_mempool_state: None,
            shutting_down: false,
            stats: Stats {
                unseen_block_count: 0,
                unseen_block_last: Instant::now(),
                unseen_block_operations_last: Instant::now(),
                applied_block_last: None,
                applied_block_level: None,
                hydrated_state_last: None,
            },
            is_sandbox,
        }
    }
}

impl Actor for ChainManager {
    type Msg = ChainManagerMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        subscribe_to_actor_terminated(ctx.system.sys_events(), ctx.myself());
        subscribe_to_network_events(&self.network_channel, ctx.myself());
        subscribe_to_shell_events(&self.shell_channel, ctx.myself());
        subscribe_to_dead_letters(ctx.system.dead_letters(), ctx.myself());

        self.hydrate_state(ctx);

        ctx.schedule::<Self::Msg, _>(
            CHECK_CHAIN_COMPLETENESS_INTERVAL / 4,
            CHECK_CHAIN_COMPLETENESS_INTERVAL,
            ctx.myself(),
            None,
            CheckChainCompleteness.into());
        ctx.schedule::<Self::Msg, _>(
            ASK_CURRENT_BRANCH_INTERVAL,
            ASK_CURRENT_BRANCH_INTERVAL,
            ctx.myself(),
            None,
            AskPeersAboutCurrentBranch.into());
        ctx.schedule::<Self::Msg, _>(
            LOG_INTERVAL / 2,
            LOG_INTERVAL,
            ctx.myself(),
            None,
            LogStats.into());

        let peer_timeout = if self.is_sandbox {
            SILENT_PEER_TIMEOUT_SANDBOX
        } else {
            SILENT_PEER_TIMEOUT / 2
        };
        ctx.schedule::<Self::Msg, _>(
            peer_timeout,
            peer_timeout,
            ctx.myself(),
            None,
            DisconnectStalledPeers.into());
    }

    fn sys_recv(&mut self, ctx: &Context<Self::Msg>, msg: SystemMsg, sender: Option<BasicActorRef>) {
        if let SystemMsg::Event(evt) = msg {
            self.receive(ctx, evt, sender);
        }
    }

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        self.receive(ctx, msg, sender);
    }
}

impl Receive<SystemEvent> for ChainManager {
    type Msg = ChainManagerMsg;

    fn receive(&mut self, _ctx: &Context<Self::Msg>, msg: SystemEvent, _sender: Option<BasicActorRef>) {
        if let SystemEvent::ActorTerminated(evt) = msg {
            if let Some(mut peer) = self.peers.remove(evt.actor.uri()) {
                peer.queued_block_headers
                    .drain()
                    .for_each(|(_, missing_block)| {
                        self.chain_state.push_missing_block(missing_block).expect("Failed to re-schedule block hash");
                    });

                self.operations_state.push_missing_block_operations(peer.queued_block_operations.drain().map(|(_, op)| op))
                    .expect("Failed to return to queue")
            }
        }
    }
}

impl Receive<DeadLetter> for ChainManager {
    type Msg = ChainManagerMsg;

    fn receive(&mut self, _ctx: &Context<Self::Msg>, msg: DeadLetter, _sender: Option<BasicActorRef>) {
        self.peers.remove(msg.recipient.uri());
    }
}

impl Receive<LogStats> for ChainManager {
    type Msg = ChainManagerMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, _msg: LogStats, _sender: Sender) {
        let log = ctx.system.log();
        let (local, local_level) = &self.current_head.local_debug_info();
        let (remote, remote_level) = &self.current_head.remote_debug_info();
        info!(log, "Head info";
            "local" => local,
            "local_level" => local_level,
            "remote" => remote,
            "remote_level" => remote_level);
        info!(log, "Blocks and operations info";
            "block_count" => self.stats.unseen_block_count,
            "last_block_secs" => self.stats.unseen_block_last.elapsed().as_secs(),
            "last_block_operations_secs" => self.stats.unseen_block_operations_last.elapsed().as_secs(),
            "applied_block_level" => self.stats.applied_block_level,
            "applied_block_secs" => self.stats.applied_block_last.map(|i| i.elapsed().as_secs()));
        for peer in self.peers.values() {
            debug!(log, "Peer state info";
                "actor_ref" => format!("{}", peer.peer_ref),
                "queued_block_headers" => peer.queued_block_headers.len(),
                "queued_block_operations" => peer.queued_block_operations.len(),
                "block_request_secs" => peer.block_request_last.elapsed().as_secs(),
                "block_response_secs" => peer.block_response_last.elapsed().as_secs(),
                "block_operations_request_secs" => peer.block_operations_request_last.elapsed().as_secs(),
                "block_operations_response_secs" => peer.block_operations_response_last.elapsed().as_secs(),
                "mempool_operations_request_secs" => peer.mempool_operations_request_last.elapsed().as_secs(),
                "mempool_operations_response_secs" => peer.mempool_operations_response_last.elapsed().as_secs(),
                "current_head_level" => peer.current_head_level,
                "current_head_update_secs" => peer.current_head_update_last.elapsed().as_secs());
        }
        info!(log, "Various info"; "peer_count" => self.peers.len(), "hydrated_state_secs" => self.stats.hydrated_state_last.map(|i| i.elapsed().as_secs()));
    }
}

impl Receive<DisconnectStalledPeers> for ChainManager {
    type Msg = ChainManagerMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, _msg: DisconnectStalledPeers, _sender: Sender) {
        self.peers.iter()
            .for_each(|(uri, state)| {
                let block_response_pending = state.block_request_last > state.block_response_last;
                let block_operations_response_pending = state.block_operations_request_last > state.block_operations_response_last;
                let mempool_operations_response_pending = state.mempool_operations_request_last > state.mempool_operations_response_last;

                let should_disconnect = if state.current_head_update_last.elapsed() > CURRENT_HEAD_LEVEL_UPDATE_TIMEOUT {
                    warn!(ctx.system.log(), "Peer failed to update its current head"; "peer" => format!("{}", uri));
                    true
                } else if block_response_pending && (state.block_request_last - state.block_response_last > SILENT_PEER_TIMEOUT) {
                    warn!(ctx.system.log(), "Peer did not respond to our request for block on time"; "peer" => format!("{}", uri), "request_secs" => state.block_request_last.elapsed().as_secs(), "response_secs" => state.block_response_last.elapsed().as_secs());
                    true
                } else if block_operations_response_pending && (state.block_operations_request_last - state.block_operations_response_last > SILENT_PEER_TIMEOUT) {
                    warn!(ctx.system.log(), "Peer did not respond to our request for block operations on time"; "peer" => format!("{}", uri), "request_secs" => state.block_operations_request_last.elapsed().as_secs(), "response_secs" => state.block_operations_response_last.elapsed().as_secs());
                    true
                } else if block_response_pending && !state.queued_block_headers.is_empty() && (state.block_response_last.elapsed() > SILENT_PEER_TIMEOUT) {
                    warn!(ctx.system.log(), "Peer is not providing requested blocks"; "peer" => format!("{}", uri), "queued_count" => state.queued_block_headers.len(), "response_secs" => state.block_response_last.elapsed().as_secs());
                    true
                } else if block_operations_response_pending && !state.queued_block_operations.is_empty() && (state.block_operations_response_last.elapsed() > SILENT_PEER_TIMEOUT) {
                    warn!(ctx.system.log(), "Peer is not providing requested block operations"; "peer" => format!("{}", uri), "queued_count" => state.queued_block_operations.len(), "response_secs" => state.block_operations_response_last.elapsed().as_secs());
                    true
                } else if mempool_operations_response_pending && !state.queued_mempool_operations.is_empty() && (state.mempool_operations_response_last.elapsed() > SILENT_PEER_TIMEOUT) {
                    warn!(ctx.system.log(), "Peer is not providing requested mempool operations"; "peer" => format!("{}", uri), "queued_count" => state.queued_mempool_operations.len(), "response_secs" => state.mempool_operations_response_last.elapsed().as_secs());
                    true
                } else {
                    false
                };

                if should_disconnect {
                    ctx.system.stop(state.peer_ref.clone());
                }
            });
    }
}

impl Receive<CheckMempoolCompleteness> for ChainManager {
    type Msg = ChainManagerMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, _msg: CheckMempoolCompleteness, _sender: Sender) {
        if !self.shutting_down {
            self.check_mempool_completeness(ctx)
        }
    }
}

impl Receive<CheckChainCompleteness> for ChainManager {
    type Msg = ChainManagerMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, _msg: CheckChainCompleteness, _sender: Sender) {
        if self.shutting_down {
            return;
        }

        match self.check_chain_completeness(ctx) {
            Ok(_) => (),
            Err(e) => warn!(ctx.system.log(), "Failed to check chain completeness"; "reason" => format!("{:?}", e)),
        }
    }
}

impl Receive<NetworkChannelMsg> for ChainManager {
    type Msg = ChainManagerMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: NetworkChannelMsg, _sender: Sender) {
        match self.process_network_channel_message(ctx, msg) {
            Ok(_) => (),
            Err(e) => warn!(ctx.system.log(), "Failed to process network channel message"; "reason" => format!("{:?}", e)),
        }
    }
}

impl Receive<ShellChannelMsg> for ChainManager {
    type Msg = ChainManagerMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: ShellChannelMsg, _sender: Sender) {
        match self.process_shell_channel_message(ctx, msg) {
            Ok(_) => (),
            Err(e) => warn!(ctx.system.log(), "Failed to process shell channel message"; "reason" => format!("{:?}", e)),
        }
    }
}

impl Receive<AskPeersAboutCurrentBranch> for ChainManager {
    type Msg = ChainManagerMsg;

    fn receive(&mut self, _ctx: &Context<Self::Msg>, _msg: AskPeersAboutCurrentBranch, _sender: Sender) {
        let ChainManager { peers, chain_state, .. } = self;
        peers.iter_mut()
            .for_each(|(_, peer)| tell_peer(GetCurrentBranchMessage::new(chain_state.get_chain_id().clone()).into(), peer))
    }
}

/// Holds information about a specific peer.
struct PeerState {
    /// Reference to peer actor
    peer_ref: PeerRef,
    // Has peer enabled mempool
    mempool_enabled: bool,

    /// Queued blocks
    queued_block_headers: HashMap<BlockHash, MissingBlock>,
    /// Queued block operations
    queued_block_operations: HashMap<BlockHash, MissingOperations>,
    /// Level of the current head received from peer
    current_head_level: Option<i32>,
    /// Last time we received updated head from peer
    current_head_update_last: Instant,
    /// Last time we requested block from the peer
    block_request_last: Instant,
    /// Last time we received block from the peer
    block_response_last: Instant,
    /// Last time we requested block operations from the peer
    block_operations_request_last: Instant,
    /// Last time we received block operations from the peer
    block_operations_response_last: Instant,
    /// Last time we requested mempool operations from the peer
    mempool_operations_request_last: Instant,
    /// Last time we received mempool operations from the peer
    mempool_operations_response_last: Instant,

    /// Missing mempool operation hashes. Peer will be asked to provide operations for those hashes.
    /// After peer is asked for operation, this hash will be moved to `queued_mempool_operations`.
    missing_mempool_operations: Vec<(OperationHash, MempoolOperationType)>,
    /// Queued mempool operations. This map holds an operation hash and
    /// a tuple of type of a mempool operation with its time to live.
    queued_mempool_operations: HashMap<OperationHash, (MempoolOperationType, SystemTime)>,
}

impl PeerState {
    fn new(peer_ref: PeerRef, peer_metadata: MetadataMessage) -> Self {
        PeerState {
            peer_ref,
            mempool_enabled: !peer_metadata.disable_mempool(),
            queued_block_headers: HashMap::new(),
            queued_block_operations: HashMap::new(),
            missing_mempool_operations: Vec::new(),
            queued_mempool_operations: HashMap::default(),
            current_head_level: None,
            current_head_update_last: Instant::now(),
            block_request_last: Instant::now(),
            block_response_last: Instant::now(),
            block_operations_request_last: Instant::now(),
            block_operations_response_last: Instant::now(),
            mempool_operations_request_last: Instant::now(),
            mempool_operations_response_last: Instant::now(),
        }
    }

    fn available_block_queue_capacity(&self) -> usize {
        let queued_count = self.queued_block_headers.len();
        if queued_count < BLOCK_HEADERS_BATCH_SIZE {
            BLOCK_HEADERS_BATCH_SIZE - queued_count
        } else {
            0
        }
    }

    fn available_block_operations_queue_capacity(&self) -> usize {
        let queued_count = self.queued_block_operations.len();
        if queued_count < BLOCK_OPERATIONS_BATCH_SIZE {
            BLOCK_OPERATIONS_BATCH_SIZE - queued_count
        } else {
            0
        }
    }

    fn available_mempool_operations_queue_capacity(&self) -> usize {
        let queued_count = self.queued_mempool_operations.len();
        if queued_count < MEMPOOL_OPERATIONS_BATCH_SIZE {
            MEMPOOL_OPERATIONS_BATCH_SIZE - queued_count
        } else {
            0
        }
    }
}

fn tell_peer(msg: PeerMessageResponse, peer: &mut PeerState) {
    peer.peer_ref.tell(SendMessage::new(msg), None);
}

fn resolve_mempool_to_send(mempool_state: &CurrentMempoolState) -> Mempool {
    // collect for mempool
    let known_valid = mempool_state.result.applied.iter().map(|a| a.hash.clone()).collect::<Vec<OperationHash>>();
    let pending = mempool_state.pending.iter().cloned().collect::<Vec<OperationHash>>();

    Mempool::new(known_valid, pending)
}

fn resolve_mempool_to_send_to_peer(peer: &PeerState, mempool_state: &Option<CurrentMempoolState>, current_head: &Head) -> Mempool {
    if !peer.mempool_enabled {
        return Mempool::default();
    }

    if let Some(mempool_state) = mempool_state {
        if let Some(mempool_head) = &mempool_state.head {
            if mempool_head.hash == current_head.hash {
                resolve_mempool_to_send(mempool_state)
            } else {
                Mempool::default()
            }
        } else {
            Mempool::default()
        }
    } else {
        Mempool::default()
    }
}