use std::cmp;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use log::{trace, debug, warn};
use riker::actors::*;

use networking::p2p::encoding::prelude::*;
use networking::p2p::network_channel::NetworkChannelMsg;
use networking::p2p::peer::{PeerRef, SendMessage};
use storage::{BlockHeaderWithHash, BlockState, MissingOperations, OperationsState};
use tezos_encoding::hash::{HashEncoding, HashRef, HashType, ToHashRef};

use crate::{subscribe_to_actor_terminated, subscribe_to_network_events};

const PEER_QUEUE_MAX: usize = 15;
const BLOCK_HEADERS_BATCH_SIZE: usize = 10;

#[derive(Clone, Debug)]
pub struct CheckChainCompleteness;

#[actor(CheckChainCompleteness, NetworkChannelMsg, SystemEvent)]
pub struct ChainManager {
    /// All events generated by the peer will end up in this channel
    event_channel: ChannelRef<NetworkChannelMsg>,
    /// Holds the state of all peers
    peers: HashMap<ActorUri, PeerState>,
    /// Holds state of the block chain
    block_state: BlockState,
    /// Holds state of the operations
    operations_state: OperationsState,
}

pub type ChainManagerRef = ActorRef<ChainManagerMsg>;

impl ChainManager {
    pub fn actor(sys: &impl ActorRefFactory, event_channel: ChannelRef<NetworkChannelMsg>, rocks_db: Arc<rocksdb::DB>) -> Result<ChainManagerRef, CreateError> {
        sys.actor_of(
            Props::new_args(ChainManager::new, (event_channel, rocks_db)),
            ChainManager::name())
    }

    /// The `ChainManager` is intended to serve as a singleton actor so that's why
    /// we won't support multiple names per instance.
    fn name() -> &'static str {
        "chain-manager"
    }

    fn new((event_channel, rocks_db): (ChannelRef<NetworkChannelMsg>, Arc<rocksdb::DB>)) -> Self {
        ChainManager {
            event_channel,
            block_state: BlockState::new(rocks_db.clone()),
            operations_state: OperationsState::new(rocks_db.clone(), rocks_db),
            peers: HashMap::new()
        }
    }
}

impl Actor for ChainManager {
    type Msg = ChainManagerMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        subscribe_to_actor_terminated(ctx.system.sys_events(), ctx.myself());
        subscribe_to_network_events(&self.event_channel, ctx.myself());

        ctx.schedule::<Self::Msg, _>(
            Duration::from_secs(15),
            Duration::from_secs(60),
            ctx.myself(),
            None,
            CheckChainCompleteness.into());
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
                    .for_each(|block_hash| {
                        self.block_state.schedule_block_hash(block_hash).expect("Failed to re-schedule block hash");
                    });

                self.operations_state.return_from_queue(peer.queued_operations.drain().map(|(_, op)| op))
                    .expect("Failed to return to queue")
            }
        }
    }
}

impl Receive<CheckChainCompleteness> for ChainManager {
    type Msg = ChainManagerMsg;

    fn receive(&mut self, _ctx: &Context<Self::Msg>, _msg: CheckChainCompleteness, _sender: Sender) {
        let ChainManager { peers, block_state, operations_state, .. } = self;

        if block_state.has_missing_blocks() {
            peers.iter_mut()
                .for_each(|(_, peer)| {
                    let available_capacity = cmp::min(peer.available_queue_capacity(), BLOCK_HEADERS_BATCH_SIZE);
                    if available_capacity > 0 {
                        let missing_block_headers = block_state.move_to_queue(available_capacity);
                        if !missing_block_headers.is_empty() {
                            debug!("Requesting {} block headers from peer {}", missing_block_headers.len(), &peer.peer_ref);
                            peer.queued_block_headers.extend(missing_block_headers.clone());
                            let msg = GetBlockHeadersMessage { get_block_headers: missing_block_headers.iter().map(HashRef::get_hash).collect() };
                            tell_peer(msg.into(), peer);
                        }
                    }
                })
        }

        if operations_state.has_missing_operations() {
            peers.iter_mut()
                .for_each(|(_, peer)| {
                    let available_capacity = peer.available_queue_capacity();
                    if available_capacity > 0 {
                        let missing_operations = operations_state.move_to_queue(available_capacity).expect("Failed to move to queue");
                        if !missing_operations.is_empty() {
                            debug!("Requesting {} operations from peer {}", missing_operations.iter().map(|op| op.validation_passes.len()).sum::<usize>(), &peer.peer_ref);
                            missing_operations.iter()
                                .for_each(|operations| {
                                    peer.queued_operations.insert(operations.block_hash.clone(), operations.clone());
                                    let msg = GetOperationsForBlocksMessage {
                                        get_operations_for_blocks: operations.into()
                                    };
                                    tell_peer(msg.into(), peer);
                                });
                        }
                    }
                })
        }
    }
}

impl Receive<NetworkChannelMsg> for ChainManager {
    type Msg = ChainManagerMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: NetworkChannelMsg, _sender: Sender) {
        match msg {
            NetworkChannelMsg::PeerBootstrapped(msg) => {
                debug!("Requesting current branch from peer: {}", &msg.peer);
                let mut peer = PeerState::new(msg.peer);
                tell_peer(GetCurrentBranchMessage::new(genesis_chain_id()).into(), &mut peer);
                // store peer
                self.peers.insert(peer.peer_ref.uri().clone(), peer);
            }
            NetworkChannelMsg::PeerMessageReceived(received) => {
                let ChainManager { peers, block_state, operations_state, .. } = self;

                match peers.get_mut(received.peer.uri()) {
                    Some(peer) => {
                        peer.response_last = Instant::now();

                        let messages = &received.message.messages;
                        messages.iter()
                            .for_each(|message| match message {
                                PeerMessage::CurrentBranch(message) => {
                                    debug!("Received current branch from peer: {}", &received.peer);
                                    message.current_branch.history.iter()
                                        .map(|block_hash| block_hash.clone().to_hash_ref())
                                        .for_each(|block_hash| block_state.schedule_block_hash(block_hash).expect("Failed to schedule block"));
                                    // trigger CheckChainCompleteness
                                    ctx.myself().tell(CheckChainCompleteness, None);
                                }
                                PeerMessage::GetCurrentBranch(_) => {
                                    debug!("Current branch requested by peer: {}", &received.peer);
                                    // .. ignore
                                }
                                PeerMessage::BlockHeader(message) => {
                                    let block_header = BlockHeaderWithHash::new(message.block_header.clone()).unwrap();
                                    block_state.insert_block_header(block_header.clone()).expect("Failed to insert block header");
                                    if peer.queued_block_headers.remove(&block_header.hash) {
                                        debug!("Received block header from peer: {}", &received.peer);
                                        operations_state.insert_block_header(&block_header).expect("Failed to insert block header");
                                        // trigger CheckChainCompleteness
                                        ctx.myself().tell(CheckChainCompleteness, None);
                                    } else {
                                        warn!("Received unexpected block header from peer: {}", &received.peer);
                                        ctx.system.stop(received.peer.clone());
                                    }
                                }
                                PeerMessage::OperationsForBlocks(operations) => {
                                    let block_hash = operations.operations_for_block.hash.clone().to_hash_ref();
                                    let mut operations_completed = false;
                                    match peer.queued_operations.get_mut(&block_hash) {
                                        Some(missing_operations) => {
                                            if missing_operations.validation_passes.remove(&operations.operations_for_block.validation_pass) {
                                                debug!("Received operations vp #{} from peer: {}", operations.operations_for_block.validation_pass, &received.peer);
                                                operations_completed = missing_operations.validation_passes.is_empty();
                                                operations_state.insert_operations(&operations).expect("Failed to insert operations");
                                                // trigger CheckChainCompleteness
                                                ctx.myself().tell(CheckChainCompleteness, None);
                                            } else {
                                                warn!("Received unexpected validation pass from peer: {}", &received.peer);
                                                ctx.system.stop(received.peer.clone());
                                            }
                                        },
                                        None => {
                                            warn!("Received unexpected operations from peer: {}", &received.peer);
                                            ctx.system.stop(received.peer.clone());
                                        }
                                    }
                                    if operations_completed {
                                        peer.queued_operations.remove(&block_hash);
                                    }


                                }
                                _ => trace!("Ignored message: {:?}", message)
                            })
                    }
                    None => debug!("Received message for non-existing peer: {}", &received.peer)
                }
            }
            _ => (),
        }
    }
}

pub fn genesis_chain_id() -> Vec<u8> {
    HashEncoding::new(HashType::ChainId).string_to_bytes("NetXgtSLGNJvNye").unwrap()
}

struct PeerState {
    peer_ref: PeerRef,
    queued_block_headers: HashSet<HashRef>,
    queued_operations: HashMap<HashRef, MissingOperations>,
    request_last: Instant,
    response_last: Instant,
}

impl PeerState {
    fn new(peer_ref: PeerRef) -> Self {
        PeerState {
            peer_ref,
            queued_block_headers: HashSet::new(),
            queued_operations: HashMap::new(),
            request_last: Instant::now(),
            response_last: Instant::now(),
        }
    }

    fn available_queue_capacity(&self) -> usize {
        let queued_count = self.queued_block_headers.len() + self.queued_operations.len();
        if queued_count < PEER_QUEUE_MAX {
            PEER_QUEUE_MAX - queued_count
        } else {
            0
        }
    }
}



fn tell_peer(msg: PeerMessageResponse, peer: &mut PeerState) {
    peer.peer_ref.tell(SendMessage::new(msg), None);
    peer.request_last = Instant::now();
}