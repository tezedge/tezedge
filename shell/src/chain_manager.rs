// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Manages chain synchronisation process.
//! - tries to download most recent header from the other peers
//! - also supplies downloaded data to other peers
//!
//! Also responsible for:
//! -- managing attribute current head
//! -- start test chain (if needed)
//! -- validate blocks with protocol
//! -- ...

use std::cmp;
use std::collections::HashMap;
use std::ops::AddAssign;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use failure::{format_err, Error};
use itertools::Itertools;
use riker::actors::*;
use slog::{debug, info, trace, warn, Logger};

use crypto::hash::{BlockHash, ChainId, CryptoboxPublicKeyHash, HashType, OperationHash};
use crypto::seeded_step::Seed;
use networking::p2p::network_channel::{NetworkChannelMsg, NetworkChannelRef, NetworkChannelTopic};
use networking::p2p::peer::SendMessage;
use networking::PeerId;
use storage::chain_meta_storage::ChainMetaStorageReader;
use storage::mempool_storage::MempoolOperationType;
use storage::persistent::PersistentStorage;
use storage::{
    BlockHeaderWithHash, BlockMetaStorage, BlockMetaStorageReader, BlockStorage,
    BlockStorageReader, ChainMetaStorage, MempoolStorage, OperationsStorage,
    OperationsStorageReader, StorageError,
};
use tezos_api::ffi::ApplyBlockRequest;
use tezos_identity::Identity;
use tezos_messages::p2p::binary_message::MessageHash;
use tezos_messages::p2p::encoding::block_header::Level;
use tezos_messages::p2p::encoding::prelude::*;
use tezos_messages::Head;
use tezos_wrapper::TezosApiConnectionPool;

use crate::chain_feeder::{ApplyBlock, ChainFeederRef};
use crate::mempool::mempool_state::MempoolState;
use crate::mempool::CurrentMempoolStateStorageRef;
use crate::shell_channel::{
    AllBlockOperationsReceived, BlockReceived, InjectBlock, MempoolOperationReceived,
    ShellChannelMsg, ShellChannelRef, ShellChannelTopic,
};
use crate::state::block_state::{BlockAcceptanceResult, BlockchainState, HeadResult, MissingBlock};
use crate::state::operations_state::{MissingOperations, OperationsState};
use crate::stats::BlockValidationTimer;
use crate::subscription::*;
use crate::utils::{dispatch_condvar_result, CondvarResult};
use crate::{validation, PeerConnectionThreshold};

/// Limit to how many blocks to request in a batch
const BLOCK_HEADERS_BATCH_SIZE: usize = 10;
/// Limit to how many block operations to request in a batch
const BLOCK_OPERATIONS_BATCH_SIZE: usize = 10;
/// Limit to how many mempool operations to request in a batch
const MEMPOOL_OPERATIONS_BATCH_SIZE: usize = 10;
/// How often to check chain completeness
const CHECK_CHAIN_COMPLETENESS_INTERVAL: Duration = Duration::from_secs(30);
/// How often to ask all connected peers for current head
const ASK_CURRENT_HEAD_INTERVAL: Duration = Duration::from_secs(90);
/// Initial delay to ask the peers for current head
const ASK_CURRENT_HEAD_INITIAL_DELAY: Duration = Duration::from_secs(15);
/// How often to print stats in logs
const LOG_INTERVAL: Duration = Duration::from_secs(60);

/// After this time we will disconnect peer if his current head level stays the same
const CURRENT_HEAD_LEVEL_UPDATE_TIMEOUT: Duration = Duration::from_secs(120);
/// After this time peer will be disconnected if it fails to respond to our request
const SILENT_PEER_TIMEOUT: Duration = Duration::from_secs(60);
/// Maximum timeout duration in sandbox mode (do not disconnect peers in sandbox mode)
const SILENT_PEER_TIMEOUT_SANDBOX: Duration = Duration::from_secs(31_536_000);

/// After this interval we will trigger rehydrate state
const REHYDRATE_STATE_INTERVAL: Duration = Duration::from_secs(240);
/// (TODO: deprecated - see RehydrateState) <_0> - timeout for last applied block, <_1> - timeout for last downloaded block
const REHYDRATE_STATE_STALLED_CHAIN_COMPLETENESS_TIMEOUT: (Duration, Duration) =
    (Duration::from_secs(240), Duration::from_secs(30));

/// Mempool operation time to live
const MEMPOOL_OPERATION_TTL: Duration = Duration::from_secs(60);

/// Message commands [`ChainManager`] to disconnect stalled peers.
#[derive(Clone, Debug)]
pub struct DisconnectStalledPeers {
    silent_peer_timeout: Duration,
}

/// Message commands [`ChainManager`] to check completeness of the chain.
#[derive(Clone, Debug)]
pub struct CheckChainCompleteness;

/// Message commands [`ChainManager`] to check if all mempool operations were fetched from peer.
#[derive(Clone, Debug)]
pub struct CheckMempoolCompleteness;

/// Message commands [`ChainManager`] to apply completed blocks.
#[derive(Clone, Debug)]
pub struct ApplyCompletedBlock {
    block_hash: BlockHash,
    result_callback: Option<CondvarResult<(), failure::Error>>,
}

/// Message commands [`ChainManager`] to process applied block.
/// Chain_feeder propagates if block successfully validated and applied
/// This is not the same as NewCurrentHead, not every applied block is set as NewCurrentHead (reorg - several headers on same level, duplicate header ...)
#[derive(Clone, Debug)]
pub struct ProcessValidatedBlock {
    block: Arc<BlockHash>,

    // TODO: remove - just temporary stats
    roundtrip_timer: Arc<Instant>,
    validation_timer: Arc<BlockValidationTimer>,
}

impl ProcessValidatedBlock {
    pub fn new(
        block: Arc<BlockHash>,
        roundtrip_timer: Arc<Instant>,
        validation_timer: Arc<BlockValidationTimer>,
    ) -> Self {
        Self {
            block,
            roundtrip_timer,
            validation_timer,
        }
    }
}

/// TODO: This will be removed, when correct bootstrap pipeline process will be done
/// Message commands [`ChainManager`] to re-hydrate state.
#[derive(Clone, Debug)]
pub struct RehydrateState;

/// Message commands [`ChainManager`] to ask all connected peers for their current branch.
#[derive(Clone, Debug)]
pub struct AskPeersAboutCurrentBranch;

/// Message commands [`ChainManager`] to ask all connected peers for their current head.
#[derive(Clone, Debug)]
pub struct AskPeersAboutCurrentHead;

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
            Some(current_remote_head) => new_remote_level > *current_remote_head.level(),
        }
    }

    fn update_remote_head(&mut self, block_header: &BlockHeaderWithHash) {
        // TODO: maybe fitness check?
        if self.need_update_remote_level(block_header.header.level()) {
            self.remote = Some(Head::new(
                block_header.hash.clone(),
                block_header.header.level(),
                block_header.header.fitness().to_vec(),
            ));
        }
    }

    fn local_debug_info(&self) -> (String, i32, String) {
        match &self.local {
            None => ("-none-".to_string(), 0_i32, "-none-".to_string()),
            Some(head) => head.to_debug_info(),
        }
    }

    fn remote_debug_info(&self) -> (String, i32, String) {
        match &self.remote {
            None => ("-none-".to_string(), 0_i32, "-none-".to_string()),
            Some(head) => head.to_debug_info(),
        }
    }
}

/// Holds various stats with info about internal synchronization.
struct Stats {
    /// Count of received blocks
    unseen_block_count: usize,
    /// Last time when previously not seen block was received
    unseen_block_last: Instant,
    /// Last time when previously unseen operations were received
    unseen_block_operations_last: Instant,
    /// ID of the last applied block
    applied_block_level: Option<Level>,
    /// Last time a block was applied
    applied_block_last: Option<Instant>,
    /// Last time state was hydrated
    hydrated_state_last: Option<Instant>,

    /// Count of applied blocks, from last LogStats run
    applied_block_lasts_count: u32,
    /// Sum of durations of block validation with protocol from last LogStats run
    applied_block_lasts_sum_validation_timer: BlockValidationTimer,
    /// Sum of durations of roundtrip: time of fired event for validation to received response
    applied_block_lasts_sum_roundtrip_timer: Duration,
}

impl Stats {
    fn clear_applied_block_lasts(&mut self) {
        self.applied_block_lasts_count = 0;
        self.applied_block_lasts_sum_validation_timer = BlockValidationTimer::default();
        self.applied_block_lasts_sum_roundtrip_timer = Duration::new(0, 0);
    }

    fn add_block_validation_stats(
        &mut self,
        roundtrip_timer: Arc<Instant>,
        validation_timer: Arc<BlockValidationTimer>,
    ) {
        self.applied_block_lasts_count += 1;
        self.applied_block_lasts_sum_validation_timer
            .add_assign(validation_timer);
        self.applied_block_lasts_sum_roundtrip_timer = match self
            .applied_block_lasts_sum_roundtrip_timer
            .checked_add(roundtrip_timer.elapsed())
        {
            Some(result) => result,
            None => self.applied_block_lasts_sum_roundtrip_timer,
        };
    }
}

/// Purpose of this actor is to perform chain synchronization.
#[actor(
    DisconnectStalledPeers,
    CheckChainCompleteness,
    ApplyCompletedBlock,
    CheckMempoolCompleteness,
    AskPeersAboutCurrentBranch,
    AskPeersAboutCurrentHead,
    LogStats,
    NetworkChannelMsg,
    ShellChannelMsg,
    SystemEvent,
    DeadLetter,
    ProcessValidatedBlock,
    RehydrateState
)]
pub struct ChainManager {
    /// Chain feeder - actor, which is responsible to apply_block to context
    block_applier: ChainFeederRef,
    /// All events generated by the network layer will end up in this channel
    network_channel: NetworkChannelRef,
    /// All events from shell will be published to this channel
    shell_channel: ShellChannelRef,
    /// Block storage
    block_storage: Box<dyn BlockStorageReader>,
    /// Block meta storage
    block_meta_storage: Box<dyn BlockMetaStorageReader>,
    /// Chain meta storage
    chain_meta_storage: Box<dyn ChainMetaStorageReader>,
    /// Operations storage
    operations_storage: Box<dyn OperationsStorageReader>,
    /// Mempool operation storage
    mempool_storage: MempoolStorage,
    /// Holds state of the blockchain
    chain_state: BlockchainState,
    /// Holds state of the operations
    operations_state: OperationsState,

    /// Node's identity public key - e.g. used for history computation
    identity_peer_id: CryptoboxPublicKeyHash,

    /// Holds the state of all peers
    peers: HashMap<ActorUri, PeerState>,
    /// Current head information
    current_head: CurrentHead,
    /// Internal stats
    stats: Stats,
    /// Indicates that we triggered check_chain_completeness
    /// (means, waiting in actor's mailbox)
    /// this is optimization
    check_chain_completeness_triggered: AtomicBool,

    /// Holds ref to global current shared mempool state
    current_mempool_state: CurrentMempoolStateStorageRef,
    /// Indicates if mempool is disabled to propagate to p2p
    p2p_disable_mempool: bool,

    /// Indicates that system is shutting down
    shutting_down: bool,
    /// Indicates node mode
    is_sandbox: bool,
    /// Indicates that [chain_manager] is bootstrapped, which means, that can broadcast stuff (new branch, new head) to the network
    is_bootstrapped: bool,
    /// Indicates threshold for minimal count of bootstrapped peers to mark chain_manager as bootstrapped
    num_of_peers_for_bootstrap_threshold: usize,

    /// Protocol runner pool dedicated to prevalidation
    tezos_readonly_prevalidation_api: Arc<TezosApiConnectionPool>,
}

/// Reference to [chain manager](ChainManager) actor.
pub type ChainManagerRef = ActorRef<ChainManagerMsg>;

impl ChainManager {
    /// Create new actor instance.
    pub fn actor(
        sys: &ActorSystem,
        block_applier: ChainFeederRef,
        network_channel: NetworkChannelRef,
        shell_channel: ShellChannelRef,
        persistent_storage: PersistentStorage,
        tezos_readonly_prevalidation_api: Arc<TezosApiConnectionPool>,
        chain_id: ChainId,
        is_sandbox: bool,
        current_mempool_state: CurrentMempoolStateStorageRef,
        p2p_disable_mempool: bool,
        peers_threshold: &PeerConnectionThreshold,
        identity: Arc<Identity>,
    ) -> Result<ChainManagerRef, CreateError> {
        sys.actor_of_props::<ChainManager>(
            ChainManager::name(),
            Props::new_args((
                block_applier,
                network_channel,
                shell_channel,
                persistent_storage,
                tezos_readonly_prevalidation_api,
                chain_id,
                is_sandbox,
                current_mempool_state,
                p2p_disable_mempool,
                peers_threshold.num_of_peers_for_bootstrap_threshold(),
                identity.peer_id(),
            )),
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
        peers
            .values_mut()
            .filter(|peer| !peer.missing_mempool_operations.is_empty())
            .filter(|peer| peer.available_block_operations_queue_capacity() > 0)
            .for_each(|peer| {
                let num_opts_to_get = cmp::min(
                    peer.missing_mempool_operations.len(),
                    peer.available_mempool_operations_queue_capacity(),
                );
                let ops_to_enqueue = peer
                    .missing_mempool_operations
                    .drain(0..num_opts_to_get)
                    .collect::<Vec<_>>();

                let ttl = SystemTime::now() + MEMPOOL_OPERATION_TTL;
                ops_to_enqueue
                    .iter()
                    .cloned()
                    .for_each(|(op_hash, op_type)| {
                        peer.queued_mempool_operations
                            .insert(op_hash, (op_type, ttl));
                    });

                let ops_to_get = ops_to_enqueue
                    .into_iter()
                    .map(|(op_hash, _)| op_hash)
                    .collect();

                peer.mempool_operations_request_last = Instant::now();
                tell_peer(GetOperationsMessage::new(ops_to_get).into(), peer);
            });
    }

    /// Check for missing blocks in local chain copy, and schedule downloading for those blocks
    fn check_chain_completeness(&mut self) {
        let ChainManager {
            peers,
            chain_state,
            operations_state,
            current_head,
            check_chain_completeness_triggered,
            ..
        } = self;

        // check for missing blocks
        if chain_state.has_missing_blocks() {
            peers
                .values_mut()
                .filter(|peer| peer.current_head_level.is_some())
                .filter(|peer| peer.available_block_queue_capacity() > 0)
                .sorted_by_key(|peer| peer.available_block_queue_capacity())
                .rev()
                .for_each(|peer| {
                    let mut missing_blocks = chain_state.drain_missing_blocks(
                        peer.available_block_queue_capacity(),
                        peer.current_head_level.unwrap(),
                    );

                    if !missing_blocks.is_empty() && current_head.local.is_some() {
                        let queued_blocks = missing_blocks
                            .drain(..)
                            .filter_map(|missing_block| {
                                let missing_block_hash = missing_block.block_hash.clone();
                                if peer
                                    .queued_block_headers
                                    .insert(missing_block_hash.clone(), missing_block)
                                    .is_none()
                                {
                                    // block was not already present in queue
                                    Some(missing_block_hash)
                                } else {
                                    // block was already in queue
                                    None
                                }
                            })
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
            peers
                .values_mut()
                .filter(|peer| peer.current_head_level.is_some())
                .filter(|peer| peer.available_block_operations_queue_capacity() > 0)
                .sorted_by_key(|peer| peer.available_block_operations_queue_capacity())
                .rev()
                .for_each(|peer| {
                    let missing_operations = operations_state.drain_missing_block_operations(
                        peer.available_block_operations_queue_capacity(),
                        peer.current_head_level.unwrap(),
                    );
                    if !missing_operations.is_empty() {
                        let queued_operations = missing_operations
                            .iter()
                            .map(|missing_operation| {
                                if peer
                                    .queued_block_operations
                                    .insert(
                                        missing_operation.block_hash.clone(),
                                        missing_operation.clone(),
                                    )
                                    .is_none()
                                {
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
                            queued_operations.iter().for_each(|&missing_operation| {
                                tell_peer(
                                    GetOperationsForBlocksMessage::new(missing_operation.into())
                                        .into(),
                                    peer,
                                )
                            });
                        }
                    }
                });
        }

        // allow next run
        check_chain_completeness_triggered.store(false, Ordering::Release);
    }

    fn process_network_channel_message(
        &mut self,
        ctx: &Context<ChainManagerMsg>,
        msg: NetworkChannelMsg,
    ) -> Result<(), Error> {
        let ChainManager {
            peers,
            chain_state,
            operations_state,
            shell_channel,
            network_channel,
            block_storage,
            block_meta_storage,
            operations_storage,
            stats,
            mempool_storage,
            current_head,
            identity_peer_id,
            check_chain_completeness_triggered,
            ..
        } = self;

        match msg {
            NetworkChannelMsg::PeerBootstrapped(peer_id, peer_metadata, _) => {
                let peer = PeerState::new(peer_id, &peer_metadata);
                // store peer
                let actor_uri = peer.peer_id.peer_ref.uri().clone();
                self.peers.insert(actor_uri.clone(), peer);
                // retrieve mutable reference and use it as `tell_peer()` parameter
                if let Some(peer) = self.peers.get_mut(&actor_uri) {
                    tell_peer(
                        GetCurrentBranchMessage::new(chain_state.get_chain_id().clone()).into(),
                        peer,
                    );
                }
            }
            NetworkChannelMsg::PeerMessageReceived(received) => {
                match peers.get_mut(received.peer.uri()) {
                    Some(peer) => {
                        let log = ctx.system.log().new(
                            slog::o!("peer_id" => peer.peer_id.as_ref().peer_id_marker.clone()),
                        );

                        for message in received.message.messages() {
                            match message {
                                PeerMessage::CurrentBranch(message) => {
                                    // at first, check if we can accept branch or just ignore it
                                    if !chain_state.can_accept_branch(&message, &current_head.local)
                                    {
                                        let head = message.current_branch().current_head();
                                        debug!(log, "Ignoring received (low) current branch";
                                                    "branch" => HashType::BlockHash.hash_to_b58check(&head.message_hash()?),
                                                    "level" => head.level());
                                    } else {
                                        let message_current_head = BlockHeaderWithHash::new(
                                            message.current_branch().current_head().clone(),
                                        )?;

                                        // schedule to download missing branch blocks
                                        chain_state.schedule_branch_bootstrap(
                                            &message_current_head,
                                            message.current_branch().history(),
                                        )?;

                                        // update remote heads
                                        current_head.update_remote_head(&message_current_head);
                                        peer.update_current_head(&message_current_head);

                                        // notify others that new block was received
                                        shell_channel.tell(
                                            Publish {
                                                msg: BlockReceived {
                                                    hash: message_current_head.hash.clone(),
                                                    level: message_current_head.header.level(),
                                                }
                                                .into(),
                                                topic: ShellChannelTopic::ShellEvents.into(),
                                            },
                                            Some(ctx.myself().into()),
                                        );

                                        // trigger CheckChainCompleteness
                                        if !check_chain_completeness_triggered
                                            .load(Ordering::Acquire)
                                        {
                                            check_chain_completeness_triggered
                                                .store(true, Ordering::Release);
                                            ctx.myself().tell(CheckChainCompleteness, None);
                                        }
                                    }
                                }
                                PeerMessage::GetCurrentBranch(message) => {
                                    if chain_state.get_chain_id() == &message.chain_id {
                                        if let Some(current_head_local) = &current_head.local {
                                            if let Some(current_head) = block_storage
                                                .get(current_head_local.block_hash())?
                                            {
                                                // calculate history
                                                let history = chain_state.get_history(
                                                    &current_head.hash,
                                                    &Seed::new(
                                                        &identity_peer_id,
                                                        &peer.peer_id.peer_public_key_hash,
                                                    ),
                                                )?;
                                                // send message
                                                let msg = CurrentBranchMessage::new(
                                                    chain_state.get_chain_id().clone(),
                                                    CurrentBranch::new(
                                                        (*current_head.header).clone(),
                                                        history,
                                                    ),
                                                );
                                                tell_peer(msg.into(), peer);
                                            }
                                        }
                                    } else {
                                        warn!(log, "Peer is requesting current branch from unsupported chain_id"; "chain_id" => HashType::ChainId.hash_to_b58check(chain_state.get_chain_id()));
                                    }
                                }
                                PeerMessage::BlockHeader(message) => {
                                    let block_header_with_hash =
                                        BlockHeaderWithHash::new(message.block_header().clone())
                                            .unwrap();
                                    match peer
                                        .queued_block_headers
                                        .remove(&block_header_with_hash.hash)
                                    {
                                        Some(_) => {
                                            peer.block_response_last = Instant::now();
                                            Self::process_downloaded_header(
                                                block_header_with_hash,
                                                ctx.myself(),
                                                &log,
                                                chain_state,
                                                operations_state,
                                                stats,
                                                check_chain_completeness_triggered,
                                                shell_channel,
                                            )?;
                                        }
                                        None => {
                                            warn!(log, "Received unexpected block header"; "block_header_hash" => HashType::BlockHash.hash_to_b58check(&block_header_with_hash.hash));
                                        }
                                    }
                                }
                                PeerMessage::GetBlockHeaders(message) => {
                                    for block_hash in message.get_block_headers() {
                                        if let Some(block) = block_storage.get(block_hash)? {
                                            let msg: BlockHeaderMessage =
                                                (*block.header).clone().into();
                                            tell_peer(msg.into(), peer);
                                        }
                                    }
                                }
                                PeerMessage::GetCurrentHead(message) => {
                                    if chain_state.get_chain_id() == message.chain_id() {
                                        if let Some(current_head_local) = &current_head.local {
                                            if let Some(current_head) = block_storage
                                                .get(current_head_local.block_hash())?
                                            {
                                                let msg = CurrentHeadMessage::new(
                                                    chain_state.get_chain_id().clone(),
                                                    current_head.header.as_ref().clone(),
                                                    Self::resolve_mempool_to_send_to_peer(
                                                        &peer,
                                                        self.p2p_disable_mempool,
                                                        self.current_mempool_state.clone(),
                                                        &current_head_local,
                                                    )?,
                                                );
                                                tell_peer(msg.into(), peer);
                                            }
                                        }
                                    }
                                }
                                PeerMessage::OperationsForBlocks(operations) => {
                                    let block_hash =
                                        operations.operations_for_block().hash().clone();
                                    match peer.queued_block_operations.get_mut(&block_hash) {
                                        Some(missing_operations) => {
                                            let operation_was_expected =
                                                missing_operations.validation_passes.remove(
                                                    &operations
                                                        .operations_for_block()
                                                        .validation_pass(),
                                                );
                                            if operation_was_expected {
                                                peer.block_operations_response_last =
                                                    Instant::now();
                                                trace!(log, "Received operations validation pass"; "validation_pass" => operations.operations_for_block().validation_pass(), "block_header_hash" => HashType::BlockHash.hash_to_b58check(&block_hash));

                                                if operations_state
                                                    .process_block_operations(&operations)?
                                                {
                                                    // update stats
                                                    stats.unseen_block_operations_last =
                                                        Instant::now();

                                                    // notify others that new all operations for block were received
                                                    let block_meta = block_meta_storage
                                                        .get(&block_hash)?
                                                        .ok_or(StorageError::MissingKey)?;

                                                    // check if block can be applied
                                                    if chain_state.can_apply_block(
                                                        (&block_hash, &block_meta),
                                                        |_| Ok(true),
                                                    )? {
                                                        ctx.myself().tell(
                                                            ApplyCompletedBlock {
                                                                block_hash: block_hash.clone(),
                                                                result_callback: None,
                                                            },
                                                            None,
                                                        );
                                                    }

                                                    // trigger CheckChainCompleteness
                                                    if !check_chain_completeness_triggered
                                                        .load(Ordering::Acquire)
                                                    {
                                                        check_chain_completeness_triggered
                                                            .store(true, Ordering::Release);
                                                        ctx.myself()
                                                            .tell(CheckChainCompleteness, None);
                                                    }

                                                    // notify others that new all operations for block were received
                                                    shell_channel.tell(
                                                        Publish {
                                                            msg: AllBlockOperationsReceived {
                                                                hash: block_hash.clone(),
                                                                level: block_meta.level(),
                                                            }
                                                            .into(),
                                                            topic: ShellChannelTopic::ShellEvents
                                                                .into(),
                                                        },
                                                        Some(ctx.myself().into()),
                                                    );

                                                    // remove operations from queue
                                                    peer.queued_block_operations
                                                        .remove(&block_hash);
                                                }
                                            } else {
                                                warn!(log, "Received unexpected validation pass"; "validation_pass" => operations.operations_for_block().validation_pass(), "block_header_hash" => HashType::BlockHash.hash_to_b58check(&block_hash));
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
                                    // process current head only if we are bootstrapped
                                    if !self.is_bootstrapped {
                                        continue;
                                    }

                                    // check if we can accept head
                                    match chain_state.can_accept_head(
                                        &message,
                                        &current_head.local,
                                        &self.tezos_readonly_prevalidation_api.pool.get()?.api,
                                    )? {
                                        BlockAcceptanceResult::AcceptBlock => {
                                            let message_current_head = BlockHeaderWithHash::new(
                                                message.current_block_header().clone(),
                                            )?;

                                            // update remote heads
                                            current_head.update_remote_head(&message_current_head);
                                            peer.update_current_head(&message_current_head);

                                            // schedule header/operations download
                                            Self::process_downloaded_header(
                                                message_current_head,
                                                ctx.myself(),
                                                &log,
                                                chain_state,
                                                operations_state,
                                                stats,
                                                check_chain_completeness_triggered,
                                                shell_channel,
                                            )?;

                                            // schedule mempool download
                                            let peer_current_mempool = message.current_mempool();

                                            // all operations (known_valid + pending) should be added to pending and validated afterwards
                                            // enqueue mempool operations for retrieval
                                            peer_current_mempool
                                                .known_valid()
                                                .iter()
                                                .cloned()
                                                .for_each(|operation_hash| {
                                                    peer.missing_mempool_operations.push((
                                                        operation_hash,
                                                        MempoolOperationType::Pending,
                                                    ));
                                                });
                                            peer_current_mempool
                                                .pending()
                                                .iter()
                                                .cloned()
                                                .for_each(|operation_hash| {
                                                    peer.missing_mempool_operations.push((
                                                        operation_hash,
                                                        MempoolOperationType::Pending,
                                                    ));
                                                });

                                            // trigger CheckMempoolCompleteness
                                            ctx.myself().tell(CheckMempoolCompleteness, None);
                                        }
                                        BlockAcceptanceResult::IgnoreBlock => {
                                            // doing nothing
                                        }
                                        BlockAcceptanceResult::UnknownBranch => {
                                            // ask current_branch from peer
                                            tell_peer(
                                                GetCurrentBranchMessage::new(
                                                    message.chain_id().clone(),
                                                )
                                                .into(),
                                                peer,
                                            );
                                        }
                                        BlockAcceptanceResult::MutlipassValidationError(error) => {
                                            warn!(log, "Mutlipass validation error detected - blacklisting peer"; "reason" => &error);

                                            // clear peer stuff immediatelly
                                            peer.clear();

                                            // blacklist peer
                                            network_channel.tell(
                                                Publish {
                                                    msg: NetworkChannelMsg::BlacklistPeer(
                                                        peer.peer_id.clone(),
                                                        format!("{:?}", error),
                                                    ),
                                                    topic: NetworkChannelTopic::NetworkCommands
                                                        .into(),
                                                },
                                                None,
                                            );
                                        }
                                    };
                                }
                                PeerMessage::GetOperations(message) => {
                                    let requested_operations: &Vec<OperationHash> =
                                        message.get_operations();
                                    for operation_hash in requested_operations {
                                        // TODO: where to look for operations for advertised mempool?
                                        // TODO: if not found here, check regular operation storage?
                                        if let Some(found) =
                                            mempool_storage.find(&operation_hash)?
                                        {
                                            tell_peer(found.into(), peer);
                                        }
                                    }
                                }
                                PeerMessage::Operation(message) => {
                                    // handling new mempool operations here
                                    // parse operation data
                                    let operation = message.operation();
                                    let operation_hash = operation.message_hash()?;

                                    match peer.queued_mempool_operations.remove(&operation_hash) {
                                        Some((operation_type, op_ttl)) => {
                                            // do prevalidation before add the operation to mempool
                                            let result = match validation::prevalidate_operation(
                                                chain_state.get_chain_id(),
                                                &operation_hash,
                                                &operation,
                                                self.current_mempool_state.clone(),
                                                &self.tezos_readonly_prevalidation_api.pool.get()?.api,
                                                block_storage,
                                                block_meta_storage,
                                            ) {
                                                Ok(result) => result,
                                                Err(e) => match e {
                                                    validation::PrevalidateOperationError::UnknownBranch { .. }
                                                    | validation::PrevalidateOperationError::BranchNotAppliedYet { .. } => {
                                                        // here we just ignore UnknownBranch
                                                        return Ok(());
                                                    }
                                                    poe => {
                                                        // other error just propagate
                                                        return Err(format_err!("Operation from p2p ({}) was not added to mempool. Reason: {:?}", HashType::OperationHash.hash_to_b58check(&operation_hash), poe));
                                                    }
                                                }
                                            };

                                            // can accpect operation ?
                                            if !validation::can_accept_operation_from_p2p(
                                                &operation_hash,
                                                &result,
                                            ) {
                                                return Err(format_err!("Operation from p2p ({}) was not added to mempool. Reason: {:?}", HashType::OperationHash.hash_to_b58check(&operation_hash), result));
                                            }

                                            // store mempool operation
                                            peer.mempool_operations_response_last = Instant::now();
                                            mempool_storage.put(
                                                operation_type.clone(),
                                                message.clone(),
                                                op_ttl,
                                            )?;

                                            // trigger CheckMempoolCompleteness
                                            ctx.myself().tell(CheckMempoolCompleteness, None);

                                            // notify others that new operation was received
                                            shell_channel.tell(
                                                Publish {
                                                    msg: MempoolOperationReceived {
                                                        operation_hash,
                                                        operation_type,
                                                        result_callback: None,
                                                    }
                                                    .into(),
                                                    topic: ShellChannelTopic::ShellEvents.into(),
                                                },
                                                Some(ctx.myself().into()),
                                            );
                                        }
                                        None => {
                                            debug!(log, "Unexpected mempool operation received")
                                        }
                                    }
                                }
                                PeerMessage::Advertise(msg) => {
                                    // re-send command to network layer
                                    network_channel.tell(
                                        Publish {
                                            msg: NetworkChannelMsg::ProcessAdvertisedPeers(
                                                peer.peer_id.clone(),
                                                msg.clone(),
                                            ),
                                            topic: NetworkChannelTopic::NetworkCommands.into(),
                                        },
                                        None,
                                    );
                                }
                                PeerMessage::Bootstrap => {
                                    // re-send command to network layer
                                    network_channel.tell(
                                        Publish {
                                            msg: NetworkChannelMsg::SendBootstrapPeers(
                                                peer.peer_id.clone(),
                                            ),
                                            topic: NetworkChannelTopic::NetworkCommands.into(),
                                        },
                                        None,
                                    );
                                }
                                ignored_message => {
                                    trace!(log, "Ignored message"; "message" => format!("{:?}", ignored_message))
                                }
                            }
                        }
                    }
                    None => {
                        warn!(ctx.system.log(), "Received message from non-existing peer actor";
                                                "peer" => received.peer.name().to_string(),
                                                "peer_uri" => received.peer.uri().to_string());
                    }
                }
            }
            _ => (),
        }

        Ok(())
    }

    fn process_shell_channel_message(
        &mut self,
        ctx: &Context<ChainManagerMsg>,
        msg: ShellChannelMsg,
    ) -> Result<(), Error> {
        match msg {
            ShellChannelMsg::ProcessValidatedGenesisBlock(msg) => {
                self.process_applied_block(ctx, msg)?;
            }
            ShellChannelMsg::AdvertiseToP2pNewMempool(chain_id, block_hash, new_mempool) => {
                // get header and send it to p2p
                if let Some(header) = self.block_storage.get(&block_hash)? {
                    self.advertise_current_head_to_p2p(
                        &chain_id,
                        header.header,
                        new_mempool.as_ref().clone(),
                        true,
                    );
                } else {
                    return Err(format_err!(
                        "BlockHeader ({}) was not found!",
                        HashType::BlockHash.hash_to_b58check(&block_hash)
                    ));
                }
            }
            ShellChannelMsg::InjectBlock(inject_block, result_callback) => {
                self.process_injected_block(inject_block, result_callback, ctx)?;
            }
            ShellChannelMsg::RequestCurrentHead(_) => {
                let ChainManager {
                    peers, chain_state, ..
                } = self;
                let msg: Arc<PeerMessageResponse> =
                    GetCurrentHeadMessage::new(chain_state.get_chain_id().to_vec()).into();
                peers
                    .iter_mut()
                    .for_each(|(_, peer)| tell_peer(msg.clone(), peer));
            }
            ShellChannelMsg::ShuttingDown(_) => {
                self.shutting_down = true;
                unsubscribe_from_dead_letters(ctx.system.dead_letters(), ctx.myself());
            }
            _ => (),
        }

        Ok(())
    }

    fn process_downloaded_header(
        received_block: BlockHeaderWithHash,
        myself: ChainManagerRef,
        log: &Logger,
        chain_state: &mut BlockchainState,
        operations_state: &mut OperationsState,
        stats: &mut Stats,
        check_chain_completeness_triggered: &mut AtomicBool,
        shell_channel: &ShellChannelRef,
    ) -> Result<(), Error> {
        // stored header and operations
        let (block_metadata, is_new_block, are_operations_complete) = chain_state
            .process_block_header(&received_block, log)
            .and_then(|(block_metadata, is_new_block)| {
                operations_state.process_block_header(&received_block).map(
                    |are_operations_complete| {
                        (block_metadata, is_new_block, are_operations_complete)
                    },
                )
            })?;

        // check if block can be applied
        if chain_state.can_apply_block((&received_block.hash, &block_metadata), |_| {
            Ok(are_operations_complete)
        })? {
            myself.tell(
                ApplyCompletedBlock {
                    block_hash: received_block.hash.clone(),
                    result_callback: None,
                },
                None,
            );
        }

        if is_new_block {
            // update stats
            stats.unseen_block_last = Instant::now();
            stats.unseen_block_count += 1;

            // trigger CheckChainCompleteness
            // trigger CheckChainCompleteness
            if !check_chain_completeness_triggered.load(Ordering::Acquire) {
                check_chain_completeness_triggered.store(true, Ordering::Release);
                myself.tell(CheckChainCompleteness, None);
            }

            // notify others that new block was received
            shell_channel.tell(
                Publish {
                    msg: BlockReceived {
                        hash: received_block.hash,
                        level: received_block.header.level(),
                    }
                    .into(),
                    topic: ShellChannelTopic::ShellEvents.into(),
                },
                Some(myself.into()),
            );
        }

        Ok(())
    }

    fn process_injected_block(
        &mut self,
        injected_block: InjectBlock,
        result_callback: Option<CondvarResult<(), failure::Error>>,
        ctx: &Context<ChainManagerMsg>,
    ) -> Result<(), Error> {
        let InjectBlock {
            block_header: block_header_with_hash,
            operations,
            operation_paths,
        } = injected_block;
        let log = ctx.system.log().new(
            slog::o!("block" => HashType::BlockHash.hash_to_b58check(&block_header_with_hash.hash)),
        );

        // this should  allways return [is_new_block==true], as we are injecting a forged new block
        let (block_metadata, is_new_block, mut are_operations_complete) = match self
            .chain_state
            .process_block_header(&block_header_with_hash, &log)
            .and_then(|(block_metadata, is_new_block)| {
                self.operations_state
                    .process_injected_block_header(&block_header_with_hash)
                    .map(|are_operations_complete| {
                        (block_metadata, is_new_block, are_operations_complete)
                    })
            }) {
            Ok(data) => data,
            Err(e) => {
                if let Err(e) = dispatch_condvar_result(
                    result_callback,
                    || {
                        Err(format_err!(
                            "Failed to store injected block, block_hash: {}, reason: {}",
                            HashType::BlockHash.hash_to_b58check(&block_header_with_hash.hash),
                            e
                        ))
                    },
                    true,
                ) {
                    warn!(log, "Failed to dispatch result to condvar"; "reason" => format!("{}", e));
                }
                return Err(e.into());
            }
        };
        info!(log, "New block injection"; "is_new_block" => is_new_block, "level" => block_header_with_hash.header.level(), "are_operations_complete" => are_operations_complete);

        if is_new_block {
            // update stats
            self.stats.unseen_block_last = Instant::now();
            self.stats.unseen_block_count += 1;

            // notify others that new block (header) was received
            self.shell_channel.tell(
                Publish {
                    msg: BlockReceived {
                        hash: block_header_with_hash.hash.clone(),
                        level: block_header_with_hash.header.level(),
                    }
                    .into(),
                    topic: ShellChannelTopic::ShellEvents.into(),
                },
                Some(ctx.myself().into()),
            );

            // handle operations (if expecting any)
            if !are_operations_complete {
                let operations = match operations {
                    Some(operations) => operations,
                    None => {
                        if let Err(e) = dispatch_condvar_result(
                            result_callback,
                            || {
                                Err(format_err!(
                                    "Missing operations in request, block_hash: {}",
                                    HashType::BlockHash
                                        .hash_to_b58check(&block_header_with_hash.hash)
                                ))
                            },
                            true,
                        ) {
                            warn!(log, "Failed to dispatch result to condvar"; "reason" => format!("{}", e));
                        }
                        return Err(format_err!(
                            "Missing operations in request, block_hash: {}",
                            HashType::BlockHash.hash_to_b58check(&block_header_with_hash.hash)
                        ));
                    }
                };
                let op_paths = match operation_paths {
                    Some(op_paths) => op_paths,
                    None => {
                        if let Err(e) = dispatch_condvar_result(
                            result_callback,
                            || {
                                Err(format_err!(
                                    "Missing operation paths in request, block_hash: {}",
                                    HashType::BlockHash
                                        .hash_to_b58check(&block_header_with_hash.hash)
                                ))
                            },
                            true,
                        ) {
                            warn!(log, "Failed to dispatch result to condvar"; "reason" => format!("{}", e));
                        }
                        return Err(format_err!(
                            "Missing operation paths in request, block_hash: {}",
                            HashType::BlockHash.hash_to_b58check(&block_header_with_hash.hash)
                        ));
                    }
                };

                // iterate through all validation passes
                for (idx, ops) in operations.into_iter().enumerate() {
                    let opb =
                        OperationsForBlock::new(block_header_with_hash.hash.clone(), idx as i8);

                    // create OperationsForBlocksMessage - the operations are stored in DB as a OperationsForBlocksMessage per validation pass per block
                    // e.g one block -> 4 validation passes -> 4 OperationsForBlocksMessage to store for the block
                    let operation_hashes_path = match op_paths.get(idx) {
                        Some(path) => path.to_owned(),
                        None => {
                            if let Err(e) = dispatch_condvar_result(
                                result_callback,
                                || {
                                    Err(format_err!("Missing operation paths in request for index: {}, block_hash: {}", idx, HashType::BlockHash.hash_to_b58check(&block_header_with_hash.hash)))
                                },
                                true,
                            ) {
                                warn!(log, "Failed to dispatch result to condvar"; "reason" => format!("{}", e));
                            }
                            return Err(format_err!(
                                "Missing operation paths in request for index: {}, block_hash: {}",
                                idx,
                                HashType::BlockHash.hash_to_b58check(&block_header_with_hash.hash)
                            ));
                        }
                    };

                    let msg: OperationsForBlocksMessage =
                        OperationsForBlocksMessage::new(opb, operation_hashes_path, ops);

                    are_operations_complete = match self
                        .operations_state
                        .process_block_operations(&msg)
                    {
                        Ok(all_operations_received) => {
                            if all_operations_received {
                                // update stats
                                self.stats.unseen_block_operations_last = Instant::now();

                                // notify others that new all operations for block were received
                                self.shell_channel.tell(
                                    Publish {
                                        msg: AllBlockOperationsReceived {
                                            hash: block_header_with_hash.hash.clone(),
                                            level: block_metadata.level(),
                                        }
                                        .into(),
                                        topic: ShellChannelTopic::ShellEvents.into(),
                                    },
                                    Some(ctx.myself().into()),
                                );
                            }
                            all_operations_received
                        }
                        Err(e) => {
                            if let Err(e) = dispatch_condvar_result(
                                result_callback,
                                || {
                                    Err(format_err!("Failed to store injected block operations, block_hash: {}, reason: {}", HashType::BlockHash.hash_to_b58check(&block_header_with_hash.hash), e))
                                },
                                true,
                            ) {
                                warn!(log, "Failed to dispatch result to condvar"; "reason" => format!("{}", e));
                            }
                            return Err(e.into());
                        }
                    };
                }
            }

            // check if block can be applied
            match self
                .chain_state
                .can_apply_block((&block_header_with_hash.hash, &block_metadata), |_| {
                    Ok(are_operations_complete)
                }) {
                Ok(can_apply) => {
                    if can_apply {
                        ctx.myself().tell(
                            ApplyCompletedBlock {
                                block_hash: block_header_with_hash.hash.clone(),
                                result_callback,
                            },
                            None,
                        );
                    } else {
                        warn!(log, "Injected block cannot be applied - will be ignored!";
                                   "block_predecessor" => HashType::BlockHash.hash_to_b58check(&block_header_with_hash.header.predecessor()),
                                   "are_operations_complete" => are_operations_complete);
                        if let Err(e) = dispatch_condvar_result(
                            result_callback,
                            || {
                                Err(format_err!("Injected block cannot be applied - will be ignored!, block_hash: {}, are_operations_complete: {}", HashType::BlockHash.hash_to_b58check(&block_header_with_hash.hash), are_operations_complete))
                            },
                            true,
                        ) {
                            warn!(log, "Failed to dispatch result to condvar"; "reason" => format!("{}", e));
                        }
                    }
                }
                Err(e) => {
                    if let Err(e) = dispatch_condvar_result(
                        result_callback,
                        || {
                            Err(format_err!("Failed to detect if injected block can be applied, block_hash: {}, reason: {}", HashType::BlockHash.hash_to_b58check(&block_header_with_hash.hash), e))
                        },
                        true,
                    ) {
                        warn!(log, "Failed to dispatch result to condvar"; "reason" => format!("{}", e));
                    }
                    return Err(e.into());
                }
            };
        } else {
            warn!(log, "Injected duplicated block - will be ignored!");
            if let Err(e) = dispatch_condvar_result(
                result_callback,
                || {
                    Err(format_err!(
                        "Injected duplicated block - will be ignored!, block_hash: {}",
                        HashType::BlockHash.hash_to_b58check(&block_header_with_hash.hash)
                    ))
                },
                true,
            ) {
                warn!(log, "Failed to dispatch result to condvar"; "reason" => format!("{}", e));
            }
        }

        Ok(())
    }

    /// Handles validated block.
    ///
    /// This logic is equivalent to [chain_validator.ml][let on_request]
    /// if applied block is winner we need to:
    /// - set current head
    /// - set bootstrapped flag
    /// - broadcast new current head/branch to peers (if bootstrapped)
    /// - start test chain (if needed) (TODO: TE-123 - not implemented yet)
    /// - update checkpoint (TODO: TE-210 - not implemented yet)
    /// - reset mempool_prevalidator
    /// ...
    fn process_applied_block(
        &mut self,
        ctx: &Context<ChainManagerMsg>,
        validated_block: ProcessValidatedBlock,
    ) -> Result<(), Error> {
        let ProcessValidatedBlock {
            block,
            roundtrip_timer,
            validation_timer,
        } = validated_block;

        // count stats
        self.stats
            .add_block_validation_stats(roundtrip_timer, validation_timer);

        // read block
        let block = match self.block_storage.get(&block)? {
            Some(block) => Arc::new(block),
            None => {
                return Err(format_err!(
                    "Block/json_data not found for block_hash: {}",
                    HashType::BlockHash.hash_to_b58check(&block)
                ));
            }
        };

        // we try to set it as "new current head", if some means set, if none means just ignore block
        if let Some((new_head, new_head_result)) = self.chain_state.try_update_new_current_head(
            &block,
            &self.current_head.local,
            self.current_mempool_state.clone(),
        )? {
            debug!(ctx.system.log(), "New current head";
                                     "block_header_hash" => HashType::BlockHash.hash_to_b58check(new_head.block_hash()),
                                     "level" => new_head.level(),
                                     "is_bootstrapped" => self.is_bootstrapped,
                                     "result" => format!("{}", new_head_result)
            );

            // update internal state with new head
            self.update_local_current_head(new_head.clone(), &ctx.system.log());

            // notify other actors that new current head was changed
            // (this also notifies [mempool_prevalidator])
            self.shell_channel.tell(
                Publish {
                    msg: ShellChannelMsg::NewCurrentHead(new_head, block.clone()),
                    topic: ShellChannelTopic::ShellEvents.into(),
                },
                Some(ctx.myself().into()),
            );

            // broadcast new head/branch to other peers
            // we can do this, only if we are bootstrapped,
            // e.g. if we just start to bootstrap from the scratch, we dont want to spam other nodes (with higher level)
            if self.is_bootstrapped {
                match new_head_result {
                    HeadResult::BranchSwitch => {
                        self.advertise_current_branch_to_p2p(
                            self.chain_state.get_chain_id(),
                            &block,
                        )?;
                    }
                    HeadResult::HeadIncrement => {
                        self.advertise_current_head_to_p2p(
                            self.chain_state.get_chain_id(),
                            block.header.clone(),
                            Mempool::default(),
                            false,
                        );
                    }
                }
            }
        }

        // check successors, if can be applied
        self.check_successors_for_apply(ctx, &block.hash)?;

        Ok(())
    }

    fn hydrate_state(&mut self, ctx: &Context<ChainManagerMsg>) {
        info!(ctx.system.log(), "Hydrating/loading current head");
        match self
            .chain_meta_storage
            .get_current_head(self.chain_state.get_chain_id())
        {
            Ok(last_known_current_head) => self.current_head.local = last_known_current_head,
            Err(e) => {
                warn!(ctx.system.log(), "Failed to load current head (lets wait for new peers and bootstrap process)"; "reason" => e)
            }
        }

        info!(ctx.system.log(), "Hydrating block state");
        if let Err(e) = self.chain_state.hydrate() {
            warn!(ctx.system.log(), "Failed to hydrate chain state (lets wait for new peers and bootstrap process)"; "reason" => e);
        }

        info!(ctx.system.log(), "Hydrating operations state");
        if let Err(e) = self.operations_state.hydrate() {
            warn!(ctx.system.log(), "Failed to hydrate operations state (lets wait for new peers and bootstrap process)"; "reason" => e);
        }

        info!(
            ctx.system.log(),
            "Hydrating/checking current head successors (if can be applied)"
        );
        if let Some(current_head_local) = self.current_head.local.as_ref() {
            if let Err(e) = self.check_successors_for_apply(ctx, current_head_local.block_hash()) {
                warn!(ctx.system.log(), "Failed to hydrate/check successors for apply (lets wait for new peers and bootstrap process)"; "reason" => e);
            }
        }

        let (local_head, local_head_level, local_fitness) = self.current_head.local_debug_info();
        info!(
            ctx.system.log(),
            "Hydrating completed successfully";
            "local_head" => local_head,
            "local_head_level" => local_head_level,
            "local_fitness" => local_fitness,
            "missing_blocks" => self.chain_state.missing_blocks_count(),
            "missing_block_operations" => self.operations_state.missing_block_operations_count(),
        );

        self.stats.hydrated_state_last = Some(Instant::now());
    }

    /// Updates currnet local head and some stats.
    /// Also checks/sets [is_bootstrapped] flag
    fn update_local_current_head(&mut self, new_head: Head, log: &Logger) {
        let new_level = *new_head.level();
        self.current_head.local = Some(new_head);
        self.stats.applied_block_level = Some(new_level);
        self.stats.applied_block_last = Some(Instant::now());
        self.resolve_is_bootstrapped(log);
    }

    /// Resolves if chain_manager is bootstrapped,
    /// means that we have at_least <> boostrapped peers
    ///
    /// "bootstrapped peer" means, that peer.current_level <= chain_manager.current_level
    fn resolve_is_bootstrapped(&mut self, log: &Logger) {
        if self.is_bootstrapped {
            return;
        }

        // simple implementation:
        // peer is considered as bootstrapped, only if his level is less_equal to chain_manager's level
        let chain_manager_current_level = self
            .current_head
            .local
            .as_ref()
            .map(|head| head.level())
            .unwrap_or(&0);
        self.peers
            .iter_mut()
            .filter(|(_, peer_state)| !peer_state.is_bootstrapped)
            .for_each(|(_, peer_state)| {
                let peer_level = peer_state.current_head_level.unwrap_or(0);
                if peer_level > 0 && peer_level <= *chain_manager_current_level {
                    info!(log, "Peer is bootstrapped"; "peer_level" => peer_level, "chain_manager_current_level" => chain_manager_current_level);
                    peer_state.is_bootstrapped = true;
                }
            });

        // chain_manager is considered as bootstrapped, only if several
        let num_of_bootstrapped_peers = self.peers.values().filter(|p| p.is_bootstrapped).count();

        // if number of bootstrapped peers is under threshold, we can mark chain_manager as bootstrapped
        if self.num_of_peers_for_bootstrap_threshold <= num_of_bootstrapped_peers {
            self.is_bootstrapped = true;
            info!(log, "Chain manager is bootstrapped"; "num_of_peers_for_bootstrap_threshold" => self.num_of_peers_for_bootstrap_threshold, "num_of_bootstrapped_peers" => num_of_bootstrapped_peers, "reached_on_level" => chain_manager_current_level)
        }
    }

    fn check_successors_for_apply(
        &self,
        ctx: &Context<ChainManagerMsg>,
        block: &BlockHash,
    ) -> Result<(), StorageError> {
        if let Some(metadata) = self.block_meta_storage.get(&block)? {
            for successor in metadata.successors() {
                if let Some(successor_metadata) = self.block_meta_storage.get(&successor)? {
                    if self
                        .chain_state
                        .can_apply_block((&successor, &successor_metadata), |bh| {
                            self.operations_state.are_operations_complete(bh)
                        })?
                    {
                        ctx.myself().tell(
                            ApplyCompletedBlock {
                                block_hash: successor.clone(),
                                result_callback: None,
                            },
                            None,
                        );
                    }
                }
            }
        }
        Ok(())
    }

    /// This should be called by [ApplyCompletedBlock], only if we have block which can be applied [chain_state.can_apply_block]
    fn apply_completed_block(
        &mut self,
        ctx: &Context<ChainManagerMsg>,
        msg: ApplyCompletedBlock,
    ) -> Result<(), Error> {
        // check if block is already applied (not necessray here)
        match self.block_meta_storage.get(&msg.block_hash)? {
            Some(meta) => {
                if meta.is_applied() {
                    // block already applied - ok, doing nothing
                    debug!(ctx.system.log(), "Block is already applied"; "block" => HashType::BlockHash.hash_to_b58check(&msg.block_hash));
                    return Ok(());
                }
            }
            None => {
                return Err(format_err!(
                    "Block metadata not found for block_hash: {}",
                    HashType::BlockHash.hash_to_b58check(&msg.block_hash)
                ));
            }
        }

        // collect data
        let request = self.prepare_apply_request(&msg.block_hash)?;

        // ping chain_feeder
        self.block_applier.tell(
            ApplyBlock::new(
                msg.block_hash,
                request,
                msg.result_callback,
                ctx.myself(),
                Instant::now(),
            ),
            None,
        );

        Ok(())
    }

    /// Collects complete data for applying block, if not complete, return None
    fn prepare_apply_request(
        &self,
        block_hash: &BlockHash,
    ) -> Result<ApplyBlockRequest, StorageError> {
        // chain_id
        let chain_id = self.chain_state.get_chain_id();

        // get block header
        let current_head = match self.block_storage.get(block_hash)? {
            Some(block) => block,
            None => return Err(StorageError::MissingKey),
        };

        // get operations
        let operations = self.operations_storage.get_operations(block_hash)?;

        // get predecessor metadata
        let (predecessor, predecessor_additional_data) = match self
            .block_storage
            .get_with_additional_data(&current_head.header.predecessor())?
        {
            Some((predecessor, predecessor_additional_data)) => {
                (predecessor, predecessor_additional_data)
            }
            None => return Err(StorageError::MissingKey),
        };

        Ok(ApplyBlockRequest {
            chain_id: chain_id.clone(),
            block_header: (&*current_head.header).clone(),
            pred_header: (&*predecessor.header).clone(),
            operations: ApplyBlockRequest::convert_operations(operations),
            max_operations_ttl: predecessor_additional_data.max_operations_ttl() as i32,
            predecessor_block_metadata_hash: predecessor_additional_data
                .block_metadata_hash()
                .clone(),
            predecessor_ops_metadata_hash: predecessor_additional_data.ops_metadata_hash().clone(),
        })
    }

    /// Send CurrentBranch message to the p2p
    fn advertise_current_branch_to_p2p(
        &self,
        chain_id: &ChainId,
        block_header: &BlockHeaderWithHash,
    ) -> Result<(), StorageError> {
        let ChainManager {
            peers,
            chain_state,
            identity_peer_id,
            ..
        } = self;

        for peer in peers.values() {
            tell_peer(
                CurrentBranchMessage::new(
                    chain_id.clone(),
                    CurrentBranch::new(
                        block_header.header.as_ref().clone(),
                        // calculate history for each peer
                        chain_state.get_history(
                            &block_header.hash,
                            &Seed::new(&identity_peer_id, &peer.peer_id.peer_public_key_hash),
                        )?,
                    ),
                )
                .into(),
                peer,
            )
        }

        Ok(())
    }

    /// Send CurrentHead message to the p2p
    ///
    /// `ignore_msg_with_empty_mempool` - if true means: send CurrentHead, only if we have anything in mempool (just to peers with enabled mempool)
    fn advertise_current_head_to_p2p(
        &self,
        chain_id: &ChainId,
        block_header: Arc<BlockHeader>,
        mempool: Mempool,
        ignore_msg_with_empty_mempool: bool,
    ) {
        // prepare messages to prevent unnecessesery cloning of messages
        // message to peers with enabled mempool
        let (msg_for_mempool_enabled_is_mempool_empty, msg_for_mempool_enabled): (
            bool,
            Arc<PeerMessageResponse>,
        ) = {
            let current_head_msg =
                CurrentHeadMessage::new(chain_id.clone(), block_header.as_ref().clone(), {
                    // we must check, if we have allowed mempool
                    if self.p2p_disable_mempool {
                        Mempool::default()
                    } else {
                        mempool
                    }
                });
            (
                current_head_msg.current_mempool().is_empty(),
                current_head_msg.into(),
            )
        };
        // message to peers with disabled mempool
        let (msg_for_mempool_disabled_is_mempool_empty, msg_for_mempool_disabled): (
            bool,
            Arc<PeerMessageResponse>,
        ) = (
            true,
            CurrentHeadMessage::new(
                chain_id.clone(),
                block_header.as_ref().clone(),
                Mempool::default(),
            )
            .into(),
        );

        // send messsages
        self.peers.iter().for_each(|(_, peer)| {
            let (msg, msg_is_mempool_empty) = if peer.mempool_enabled {
                (
                    msg_for_mempool_enabled.clone(),
                    msg_for_mempool_enabled_is_mempool_empty,
                )
            } else {
                (
                    msg_for_mempool_disabled.clone(),
                    msg_for_mempool_disabled_is_mempool_empty,
                )
            };

            let can_send_msg = !(ignore_msg_with_empty_mempool && msg_is_mempool_empty);
            if can_send_msg {
                tell_peer(msg, peer)
            }
        });
    }

    fn resolve_mempool_to_send_to_peer(
        peer: &PeerState,
        p2p_disable_mempool: bool,
        current_mempool_state: CurrentMempoolStateStorageRef,
        current_head: &Head,
    ) -> Result<Mempool, failure::Error> {
        if p2p_disable_mempool {
            return Ok(Mempool::default());
        }
        if !peer.mempool_enabled {
            return Ok(Mempool::default());
        }

        let mempool_state = current_mempool_state
            .read()
            .map_err(|e| format_err!("Failed to lock for read, reason: {}", e))?;
        if let Some(mempool_head_hash) = mempool_state.head() {
            if mempool_head_hash == current_head.block_hash() {
                let mempool_state: &MempoolState = &mempool_state;
                Ok(mempool_state.into())
            } else {
                Ok(Mempool::default())
            }
        } else {
            Ok(Mempool::default())
        }
    }
}

impl
    ActorFactoryArgs<(
        ChainFeederRef,
        NetworkChannelRef,
        ShellChannelRef,
        PersistentStorage,
        Arc<TezosApiConnectionPool>,
        ChainId,
        bool,
        CurrentMempoolStateStorageRef,
        bool,
        usize,
        CryptoboxPublicKeyHash,
    )> for ChainManager
{
    fn create_args(
        (
            block_applier,
            network_channel,
            shell_channel,
            persistent_storage,
            tezos_readonly_prevalidation_api,
            chain_id,
            is_sandbox,
            current_mempool_state,
            p2p_disable_mempool,
            num_of_peers_for_bootstrap_threshold,
            identity_peer_id,
        ): (
            ChainFeederRef,
            NetworkChannelRef,
            ShellChannelRef,
            PersistentStorage,
            Arc<TezosApiConnectionPool>,
            ChainId,
            bool,
            CurrentMempoolStateStorageRef,
            bool,
            usize,
            CryptoboxPublicKeyHash,
        ),
    ) -> Self {
        ChainManager {
            block_applier,
            network_channel,
            shell_channel,
            block_storage: Box::new(BlockStorage::new(&persistent_storage)),
            block_meta_storage: Box::new(BlockMetaStorage::new(&persistent_storage)),
            chain_meta_storage: Box::new(ChainMetaStorage::new(&persistent_storage)),
            operations_storage: Box::new(OperationsStorage::new(&persistent_storage)),
            mempool_storage: MempoolStorage::new(&persistent_storage),
            chain_state: BlockchainState::new(&persistent_storage, chain_id.clone()),
            operations_state: OperationsState::new(&persistent_storage, chain_id),
            peers: HashMap::new(),
            current_head: CurrentHead {
                local: None,
                remote: None,
            },
            shutting_down: false,
            stats: Stats {
                unseen_block_count: 0,
                unseen_block_last: Instant::now(),
                unseen_block_operations_last: Instant::now(),
                hydrated_state_last: None,
                applied_block_level: None,
                applied_block_last: None,
                applied_block_lasts_count: 0,
                applied_block_lasts_sum_validation_timer: BlockValidationTimer::default(),
                applied_block_lasts_sum_roundtrip_timer: Duration::new(0, 0),
            },
            check_chain_completeness_triggered: AtomicBool::new(false),
            is_sandbox,
            identity_peer_id,
            is_bootstrapped: false,
            current_mempool_state,
            p2p_disable_mempool,
            num_of_peers_for_bootstrap_threshold,
            tezos_readonly_prevalidation_api,
        }
    }
}

impl Actor for ChainManager {
    type Msg = ChainManagerMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        subscribe_to_actor_terminated(ctx.system.sys_events(), ctx.myself());
        subscribe_to_dead_letters(ctx.system.dead_letters(), ctx.myself());
        subscribe_to_network_events(&self.network_channel, ctx.myself());
        subscribe_to_shell_shutdown(&self.shell_channel, ctx.myself());
        subscribe_to_shell_commands(&self.shell_channel, ctx.myself());

        ctx.schedule::<Self::Msg, _>(
            CHECK_CHAIN_COMPLETENESS_INTERVAL / 4,
            CHECK_CHAIN_COMPLETENESS_INTERVAL,
            ctx.myself(),
            None,
            CheckChainCompleteness.into(),
        );
        ctx.schedule::<Self::Msg, _>(
            ASK_CURRENT_HEAD_INITIAL_DELAY,
            ASK_CURRENT_HEAD_INTERVAL,
            ctx.myself(),
            None,
            AskPeersAboutCurrentHead.into(),
        );
        ctx.schedule::<Self::Msg, _>(
            LOG_INTERVAL / 2,
            LOG_INTERVAL,
            ctx.myself(),
            None,
            LogStats.into(),
        );
        ctx.schedule::<Self::Msg, _>(
            REHYDRATE_STATE_INTERVAL,
            REHYDRATE_STATE_INTERVAL,
            ctx.myself(),
            None,
            RehydrateState.into(),
        );

        let silent_peer_timeout = if self.is_sandbox {
            SILENT_PEER_TIMEOUT_SANDBOX
        } else {
            SILENT_PEER_TIMEOUT
        };
        ctx.schedule::<Self::Msg, _>(
            silent_peer_timeout,
            silent_peer_timeout,
            ctx.myself(),
            None,
            DisconnectStalledPeers {
                silent_peer_timeout,
            }
            .into(),
        );
    }

    fn post_start(&mut self, ctx: &Context<Self::Msg>) {
        // now we can hydrate state
        self.hydrate_state(ctx);
    }

    fn sys_recv(
        &mut self,
        ctx: &Context<Self::Msg>,
        msg: SystemMsg,
        sender: Option<BasicActorRef>,
    ) {
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

    fn receive(
        &mut self,
        _ctx: &Context<Self::Msg>,
        msg: SystemEvent,
        _sender: Option<BasicActorRef>,
    ) {
        if let SystemEvent::ActorTerminated(evt) = msg {
            if let Some(mut peer) = self.peers.remove(evt.actor.uri()) {
                peer.queued_block_headers
                    .drain()
                    .for_each(|(_, missing_block)| {
                        self.chain_state
                            .push_missing_block(missing_block)
                            .expect("Failed to re-schedule block hash");
                    });

                self.operations_state
                    .push_missing_block_operations(
                        peer.queued_block_operations.drain().map(|(_, op)| op),
                    )
                    .expect("Failed to return to queue")
            }
        }
    }
}

impl Receive<DeadLetter> for ChainManager {
    type Msg = ChainManagerMsg;

    fn receive(
        &mut self,
        _ctx: &Context<Self::Msg>,
        msg: DeadLetter,
        _sender: Option<BasicActorRef>,
    ) {
        self.peers.remove(msg.recipient.uri());
    }
}

impl Receive<LogStats> for ChainManager {
    type Msg = ChainManagerMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, _msg: LogStats, _sender: Sender) {
        let log = ctx.system.log();
        let (local, local_level, local_fitness) = &self.current_head.local_debug_info();
        let (remote, remote_level, remote_fitness) = &self.current_head.remote_debug_info();

        // calculate applied stats
        let last_applied = {
            let Stats {
                applied_block_lasts_count,
                applied_block_lasts_sum_validation_timer,
                applied_block_lasts_sum_roundtrip_timer,
                ..
            } = &self.stats;

            if *applied_block_lasts_count > 0 {
                let validation = applied_block_lasts_sum_validation_timer
                    .print_formatted_average_for_count(*applied_block_lasts_count);
                let roundtrip = match applied_block_lasts_sum_roundtrip_timer
                    .checked_div(*applied_block_lasts_count)
                {
                    Some(result) => format!("{:?}", result),
                    None => "-".to_string(),
                };

                // calculated message
                let stats = format!(
                    "({} blocks - average times [request_response {:?} -> {}]",
                    applied_block_lasts_count, roundtrip, validation,
                );

                // clear stats for next run
                self.stats.clear_applied_block_lasts();

                stats
            } else {
                format!("({} blocks)", applied_block_lasts_count)
            }
        };

        info!(log, "Head info";
            "local" => local,
            "local_level" => local_level,
            "local_fitness" => local_fitness,
            "remote" => remote,
            "remote_level" => remote_level,
            "remote_fitness" => remote_fitness);
        info!(log, "Blocks and operations info";
            "block_count" => self.stats.unseen_block_count,
            "missing_blocks" => self.chain_state.missing_blocks_count(),
            "missing_block_operations" => self.operations_state.missing_block_operations_count(),
            "last_block_secs" => self.stats.unseen_block_last.elapsed().as_secs(),
            "last_block_operations_secs" => self.stats.unseen_block_operations_last.elapsed().as_secs(),
            "applied_block_level" => self.stats.applied_block_level,
            "applied_block_secs" => self.stats.applied_block_last.map(|i| i.elapsed().as_secs()));
        for peer in self.peers.values() {
            debug!(log, "Peer state info";
                "actor_ref" => format!("{}", peer.peer_id.peer_ref),
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
        info!(log, "Various info";
                   "peer_count" => self.peers.len(),
                   "hydrated_state_secs" => self.stats.hydrated_state_last.map(|i| i.elapsed().as_secs()),
                   "local_level" => local_level,
                   "last_applied" => last_applied,
        );
    }
}

impl Receive<DisconnectStalledPeers> for ChainManager {
    type Msg = ChainManagerMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: DisconnectStalledPeers, _sender: Sender) {
        self.peers.iter()
            .for_each(|(uri, state)| {
                let block_response_pending = state.block_request_last > state.block_response_last;
                let block_operations_response_pending = state.block_operations_request_last > state.block_operations_response_last;
                let mempool_operations_response_pending = state.mempool_operations_request_last > state.mempool_operations_response_last;

                let should_disconnect = if state.current_head_update_last.elapsed() > CURRENT_HEAD_LEVEL_UPDATE_TIMEOUT {
                    warn!(ctx.system.log(), "Peer failed to update its current head";
                                            "peer_id" => state.peer_id.peer_id_marker.clone(), "peer_ip" => state.peer_id.peer_address.to_string(), "peer" => state.peer_id.peer_ref.name(), "peer_uri" => uri.to_string());
                    true
                } else if block_response_pending && (state.block_request_last - state.block_response_last > msg.silent_peer_timeout) {
                    warn!(ctx.system.log(), "Peer did not respond to our request for block on time"; "request_secs" => state.block_request_last.elapsed().as_secs(), "response_secs" => state.block_response_last.elapsed().as_secs(),
                                            "peer_id" => state.peer_id.peer_id_marker.clone(), "peer_ip" => state.peer_id.peer_address.to_string(), "peer" => state.peer_id.peer_ref.name(), "peer_uri" => uri.to_string());
                    true
                } else if block_operations_response_pending && (state.block_operations_request_last - state.block_operations_response_last > msg.silent_peer_timeout) {
                    warn!(ctx.system.log(), "Peer did not respond to our request for block operations on time"; "request_secs" => state.block_operations_request_last.elapsed().as_secs(), "response_secs" => state.block_operations_response_last.elapsed().as_secs(),
                                            "peer_id" => state.peer_id.peer_id_marker.clone(), "peer_ip" => state.peer_id.peer_address.to_string(), "peer" => state.peer_id.peer_ref.name(), "peer_uri" => uri.to_string());
                    true
                } else if block_response_pending && !state.queued_block_headers.is_empty() && (state.block_response_last.elapsed() > msg.silent_peer_timeout) {
                    warn!(ctx.system.log(), "Peer is not providing requested blocks"; "queued_count" => state.queued_block_headers.len(), "response_secs" => state.block_response_last.elapsed().as_secs(),
                                            "peer_id" => state.peer_id.peer_id_marker.clone(), "peer_ip" => state.peer_id.peer_address.to_string(), "peer" => state.peer_id.peer_ref.name(), "peer_uri" => uri.to_string());
                    true
                } else if block_operations_response_pending && !state.queued_block_operations.is_empty() && (state.block_operations_response_last.elapsed() > msg.silent_peer_timeout) {
                    warn!(ctx.system.log(), "Peer is not providing requested block operations"; "queued_count" => state.queued_block_operations.len(), "response_secs" => state.block_operations_response_last.elapsed().as_secs(),
                                            "peer_id" => state.peer_id.peer_id_marker.clone(), "peer_ip" => state.peer_id.peer_address.to_string(), "peer" => state.peer_id.peer_ref.name(), "peer_uri" => uri.to_string());
                    true
                } else if mempool_operations_response_pending && !state.queued_mempool_operations.is_empty() && (state.mempool_operations_response_last.elapsed() > msg.silent_peer_timeout) {
                    warn!(ctx.system.log(), "Peer is not providing requested mempool operations"; "queued_count" => state.queued_mempool_operations.len(), "response_secs" => state.mempool_operations_response_last.elapsed().as_secs(),
                                            "peer_id" => state.peer_id.peer_id_marker.clone(), "peer_ip" => state.peer_id.peer_address.to_string(), "peer" => state.peer_id.peer_ref.name(), "peer_uri" => uri.to_string());
                    true
                } else {
                    false
                };

                if should_disconnect {
                    ctx.system.stop(state.peer_id.peer_ref.clone());
                }
            });
    }
}

impl Receive<CheckMempoolCompleteness> for ChainManager {
    type Msg = ChainManagerMsg;

    fn receive(
        &mut self,
        ctx: &Context<Self::Msg>,
        _msg: CheckMempoolCompleteness,
        _sender: Sender,
    ) {
        if !self.shutting_down {
            self.check_mempool_completeness(ctx)
        }
    }
}

impl Receive<CheckChainCompleteness> for ChainManager {
    type Msg = ChainManagerMsg;

    fn receive(&mut self, _: &Context<Self::Msg>, _msg: CheckChainCompleteness, _sender: Sender) {
        if self.shutting_down {
            return;
        }
        self.check_chain_completeness()
    }
}

impl Receive<ApplyCompletedBlock> for ChainManager {
    type Msg = ChainManagerMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: ApplyCompletedBlock, _sender: Sender) {
        if self.shutting_down {
            return;
        }

        match self.apply_completed_block(ctx, msg) {
            Ok(_) => (),
            Err(e) => {
                warn!(ctx.system.log(), "Failed to apply completed block"; "reason" => format!("{:?}", e))
            }
        }
    }
}

impl Receive<NetworkChannelMsg> for ChainManager {
    type Msg = ChainManagerMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: NetworkChannelMsg, _sender: Sender) {
        match self.process_network_channel_message(ctx, msg) {
            Ok(_) => (),
            Err(e) => {
                warn!(ctx.system.log(), "Failed to process network channel message"; "reason" => format!("{:?}", e))
            }
        }
    }
}

impl Receive<RehydrateState> for ChainManager {
    type Msg = ChainManagerMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, _msg: RehydrateState, _sender: Sender) {
        if self.shutting_down {
            return;
        }

        if let (Some(applied_block_last), unseen_block_last) =
            (self.stats.applied_block_last, self.stats.unseen_block_last)
        {
            // TODO: all this RehydrateState event will be removed
            let last_applied_block_timeout_elapsed =
                applied_block_last.elapsed() > REHYDRATE_STATE_STALLED_CHAIN_COMPLETENESS_TIMEOUT.0;
            let last_downloaded_block_timeout_elapsed =
                unseen_block_last.elapsed() > REHYDRATE_STATE_STALLED_CHAIN_COMPLETENESS_TIMEOUT.1;

            // we rehydrate state, only if we "did not apply block for a long time" and "did not download block for a long time"
            if last_applied_block_timeout_elapsed && last_downloaded_block_timeout_elapsed {
                self.hydrate_state(ctx);
            }
        }
    }
}

impl Receive<ShellChannelMsg> for ChainManager {
    type Msg = ChainManagerMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: ShellChannelMsg, _sender: Sender) {
        match self.process_shell_channel_message(ctx, msg) {
            Ok(_) => (),
            Err(e) => {
                warn!(ctx.system.log(), "Failed to process shell channel message"; "reason" => format!("{:?}", e))
            }
        }
    }
}

impl Receive<ProcessValidatedBlock> for ChainManager {
    type Msg = ChainManagerMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: ProcessValidatedBlock, _: Sender) {
        match self.process_applied_block(ctx, msg) {
            Ok(_) => (),
            Err(e) => {
                warn!(ctx.system.log(), "Failed to process validated block"; "reason" => format!("{:?}", e))
            }
        }
    }
}

impl Receive<AskPeersAboutCurrentBranch> for ChainManager {
    type Msg = ChainManagerMsg;

    fn receive(
        &mut self,
        _ctx: &Context<Self::Msg>,
        _msg: AskPeersAboutCurrentBranch,
        _sender: Sender,
    ) {
        let ChainManager {
            peers, chain_state, ..
        } = self;
        peers.iter_mut().for_each(|(_, peer)| {
            tell_peer(
                GetCurrentBranchMessage::new(chain_state.get_chain_id().clone()).into(),
                peer,
            )
        })
    }
}

impl Receive<AskPeersAboutCurrentHead> for ChainManager {
    type Msg = ChainManagerMsg;

    fn receive(
        &mut self,
        _ctx: &Context<Self::Msg>,
        _msg: AskPeersAboutCurrentHead,
        _sender: Sender,
    ) {
        let ChainManager {
            peers, chain_state, ..
        } = self;
        peers.iter_mut().for_each(|(_, peer)| {
            tell_peer(
                GetCurrentHeadMessage::new(chain_state.get_chain_id().clone()).into(),
                peer,
            )
        })
    }
}

/// Holds information about a specific peer.
struct PeerState {
    /// PeerId identification (actor_ref + public key)
    peer_id: Arc<PeerId>,
    /// Has peer enabled mempool
    mempool_enabled: bool,
    /// Is bootstrapped flag
    is_bootstrapped: bool,

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
    fn new(peer_id: Arc<PeerId>, peer_metadata: &MetadataMessage) -> Self {
        PeerState {
            peer_id,
            mempool_enabled: !peer_metadata.disable_mempool(),
            is_bootstrapped: false,
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

    fn update_current_head(&mut self, block_header: &BlockHeaderWithHash) {
        // TODO: maybe fitness check?
        if self.current_head_level.is_none()
            || (block_header.header.level() > self.current_head_level.unwrap())
        {
            self.current_head_level = Some(block_header.header.level());
            self.current_head_update_last = Instant::now();
        }
    }

    fn clear(&mut self) {
        self.missing_mempool_operations.clear();
        self.queued_block_headers.clear();
        self.queued_block_operations.clear();
        self.queued_mempool_operations.clear();
    }
}

fn tell_peer(msg: Arc<PeerMessageResponse>, peer: &PeerState) {
    peer.peer_id.peer_ref.tell(SendMessage::new(msg), None);
}

#[cfg(test)]
pub mod tests {
    use std::net::SocketAddr;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc::channel;
    use std::sync::Mutex;
    use std::thread;

    use slog::{Drain, Level, Logger};

    use crypto::hash::CryptoboxPublicKeyHash;
    use networking::p2p::network_channel::NetworkChannel;
    use networking::p2p::peer::Peer;
    use networking::ShellCompatibilityVersion;
    use storage::tests_common::TmpStorage;
    use tezos_api::environment::{TezosEnvironment, TezosEnvironmentConfiguration, TEZOS_ENV};
    use tezos_api::ffi::TezosRuntimeConfiguration;
    use tezos_wrapper::ProtocolEndpointConfiguration;
    use tezos_wrapper::TezosApiConnectionPoolConfiguration;

    use crate::chain_feeder::ChainFeeder;
    use crate::mempool::init_mempool_state_storage;
    use crate::shell_channel::{ShellChannel, ShuttingDown};

    use super::*;

    fn create_logger(level: Level) -> Logger {
        let drain = slog_async::Async::new(
            slog_term::FullFormat::new(slog_term::TermDecorator::new().build())
                .build()
                .fuse(),
        )
        .build()
        .filter_level(level)
        .fuse();

        Logger::root(drain, slog::o!())
    }

    fn create_tokio_runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to create tokio runtime")
    }

    fn peer(
        sys: &impl ActorRefFactory,
        network_channel: NetworkChannelRef,
        tokio_runtime: &tokio::runtime::Runtime,
    ) -> PeerState {
        let socket_address: SocketAddr = "127.0.0.1:3011"
            .parse()
            .expect("Expected valid ip:port address");

        let node_identity = Arc::new(Identity::generate(0f64));
        let peer_public_key_hash: CryptoboxPublicKeyHash =
            node_identity.public_key.public_key_hash();
        let peer_id_marker =
            HashType::CryptoboxPublicKeyHash.hash_to_b58check(&peer_public_key_hash);

        let peer_ref = Peer::actor(
            sys,
            network_channel,
            3011,
            node_identity,
            Arc::new(ShellCompatibilityVersion::new(
                "testet".to_string(),
                vec![0],
                vec![0],
            )),
            tokio_runtime.handle().clone(),
            &socket_address,
        )
        .unwrap();

        PeerState::new(
            Arc::new(PeerId::new(
                peer_ref,
                peer_public_key_hash,
                peer_id_marker,
                socket_address,
            )),
            &MetadataMessage::new(false, false),
        )
    }

    fn assert_peer_bootstrapped(
        chain_manager: &mut ChainManager,
        peer_uri: &ActorUri,
        expected_is_bootstrap: bool,
    ) {
        let peer_state = chain_manager.peers.get(&peer_uri).unwrap();
        assert_eq!(expected_is_bootstrap, peer_state.is_bootstrapped);
    }

    #[test]
    fn test_resolve_is_bootstrapped() -> Result<(), Error> {
        let log = create_logger(Level::Debug);
        let storage = TmpStorage::create_to_out_dir("__test_resolve_is_bootstrapped")?;

        let tokio_runtime = create_tokio_runtime();
        let actor_system = SystemBuilder::new()
            .name("test_actors_apply_blocks_and_check_context")
            .log(log.clone())
            .create()
            .expect("Failed to create actor system");
        let shell_channel =
            ShellChannel::actor(&actor_system).expect("Failed to create shell channel");
        let network_channel =
            NetworkChannel::actor(&actor_system).expect("Failed to create network channel");
        let chain_id = HashType::ChainId.b58check_to_hash("NetXgtSLGNJvNye")?;
        let tezos_env: &TezosEnvironmentConfiguration = TEZOS_ENV
            .get(&TezosEnvironment::Sandbox)
            .expect("no environment configuration");

        let pool = Arc::new(TezosApiConnectionPool::new_without_context(
            String::from("test_pool"),
            TezosApiConnectionPoolConfiguration {
                connection_timeout: Duration::from_secs(1),
                idle_timeout: Duration::from_secs(1),
                max_lifetime: Duration::from_secs(1),
                min_connections: 0,
                max_connections: 1,
            },
            ProtocolEndpointConfiguration::new(
                TezosRuntimeConfiguration {
                    log_enabled: false,
                    no_of_ffi_calls_treshold_for_gc: 0,
                    debug_mode: false,
                },
                tezos_env.clone(),
                false,
                "__test_resolve_is_bootstrapped/context",
                "--no-executable-needed-here--",
                Level::Debug,
                false,
            ),
            log.clone(),
        ));

        // chain feeder mock
        let block_applier_run = Arc::new(AtomicBool::new(true));
        let block_applier = {
            let block_applier_thread = thread::spawn(move || -> Result<(), Error> { Ok(()) });
            let (block_applier_event_sender, _) = channel();

            actor_system.actor_of_props::<ChainFeeder>(
                "chain-feeder-mock",
                Props::new_args((
                    shell_channel.clone(),
                    block_applier_run.clone(),
                    Arc::new(Mutex::new(Some(block_applier_thread))),
                    Arc::new(Mutex::new(block_applier_event_sender)),
                )),
            )?
        };

        // direct instance of ChainManager (not throught actor_system)
        let mut chain_manager = ChainManager::create_args((
            block_applier,
            network_channel.clone(),
            shell_channel.clone(),
            storage.storage().clone(),
            pool,
            chain_id,
            false,
            init_mempool_state_storage(),
            false,
            1,
            tezos_identity::Identity::generate(0f64).peer_id(),
        ));

        // empty chain_manager
        chain_manager.resolve_is_bootstrapped(&log);
        assert!(!chain_manager.is_bootstrapped);

        // add one not bootstrapped peer with level 0
        let mut peer_state = peer(&actor_system, network_channel.clone(), &tokio_runtime);
        peer_state.current_head_level = Some(0);
        let peer_key = peer_state.peer_id.peer_ref.uri().clone();
        chain_manager.peers.insert(peer_key, peer_state);

        // add one not bootstrapped peer with level 5
        let mut peer_state = peer(&actor_system, network_channel, &tokio_runtime);
        peer_state.current_head_level = Some(5);
        let peer_key = peer_state.peer_id.peer_ref.uri().clone();
        chain_manager.peers.insert(peer_key.clone(), peer_state);

        // check chain_manager and peer (not bootstrapped)
        chain_manager.resolve_is_bootstrapped(&log);
        assert!(!chain_manager.is_bootstrapped);
        assert_peer_bootstrapped(&mut chain_manager, &peer_key, false);

        // simulate - block applied event with level 4
        let new_head = Head::new(
            HashType::BlockHash
                .b58check_to_hash("BLFQ2JjYWHC95Db21cRZC4cgyA1mcXmx1Eg6jKywWy9b8xLzyK9")?,
            4,
            vec![],
        );
        chain_manager.update_local_current_head(new_head, &log);

        // check chain_manager and peer (not bootstrapped)
        assert!(!chain_manager.is_bootstrapped);
        assert_peer_bootstrapped(&mut chain_manager, &peer_key, false);

        // simulate - block applied event with level 5
        let new_head = Head::new(
            HashType::BlockHash
                .b58check_to_hash("BLFQ2JjYWHC95Db21cRZC4cgyA1mcXmx1Eg6jKywWy9b8xLzyK9")?,
            5,
            vec![],
        );
        chain_manager.update_local_current_head(new_head, &log);

        // check chain_manager and peer (should be bootstrapped now)
        assert!(chain_manager.is_bootstrapped);
        assert_peer_bootstrapped(&mut chain_manager, &peer_key, true);

        // close
        block_applier_run.store(false, Ordering::Release);
        shell_channel.tell(
            Publish {
                msg: ShuttingDown.into(),
                topic: ShellChannelTopic::ShellShutdown.into(),
            },
            None,
        );
        let _ = tokio_runtime.block_on(actor_system.shutdown());

        Ok(())
    }
}
