use slog::Drain;
use std::collections::{HashSet, HashMap, VecDeque};
use std::iter::FromIterator;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tezedge_state::ShellCompatibilityVersion;
use tezos_messages::p2p::encoding::peer::{PeerMessage, PeerMessageResponse};
use tla_sm::Acceptor;

use crypto::{
    crypto_box::{CryptoKey, PublicKey, SecretKey},
    hash::{CryptoboxPublicKeyHash, HashTrait},
    proof_of_work::ProofOfWork,
};
use hex::FromHex;
use tezedge_state::proposals::ExtendPotentialPeersProposal;
use tezedge_state::{PeerAddress, TezedgeConfig, TezedgeState};
use tezos_identity::Identity;

use tezedge_state::proposer::mio_manager::{MioEvents, MioManager};
use tezedge_state::proposer::{Notification, TezedgeProposer, TezedgeProposerConfig};
use tezos_messages::p2p::encoding::prelude::{CurrentBranchMessage, BlockHeader, CurrentBranch, GetCurrentBranchMessage, GetBlockHeadersMessage, GetCurrentHeadMessage};
use std::convert::TryInto;
use crypto::hash::{ContextHash, OperationListListHash, BlockHash, chain_id_from_block_hash, ChainId};
use tezos_api::environment;
use tezos_api::environment::{TezosEnvironment, get_empty_operation_list_list_hash};
use std::net::IpAddr;
use storage::{BlockStorage, BlockMetaStorage, PersistentStorage, BlockHeaderWithHash};
use storage::persistent::{open_cl, CommitLogSchema};
use storage::persistent::sequence::Sequences;
use storage::database::tezedge_database::{TezedgeDatabase, TezedgeDatabaseBackendOptions};
use storage::database::notus_backend::NotusDBBackend;
use tezos_messages::p2p::encoding::block_header::Level;
use tezos_messages::p2p::binary_message::MessageHash;
use std::thread::yield_now;
use std::process::exit;
use tezos_messages::p2p::encoding::peer::PeerMessage::GetCurrentHead;

const CHAIN_NAME : &'static str = "TEZOS_MAINNET";

fn shell_compatibility_version() -> ShellCompatibilityVersion {
    ShellCompatibilityVersion::new(CHAIN_NAME.to_owned(), vec![0], vec![0, 1])
}

fn identity(pkh: &[u8], pk: &[u8], sk: &[u8], pow: &[u8]) -> Identity {
    Identity {
        peer_id: CryptoboxPublicKeyHash::try_from_bytes(pkh).unwrap(),
        public_key: PublicKey::from_bytes(pk).unwrap(),
        secret_key: SecretKey::from_bytes(sk).unwrap(),
        proof_of_work_stamp: ProofOfWork::from_hex(hex::encode(pow)).unwrap(),
    }
}

fn identity_1() -> Identity {
    identity(
        &[
            86, 205, 231, 178, 152, 146, 2, 157, 213, 131, 90, 117, 83, 132, 177, 84,
        ],
        &[
            148, 73, 141, 148, 22, 20, 15, 188, 69, 132, 149, 51, 61, 170, 193, 180, 200, 126, 65,
            159, 87, 38, 113, 122, 84, 249, 182, 198, 116, 118, 174, 28,
        ],
        &[
            172, 122, 207, 58, 254, 215, 99, 123, 225, 15, 143, 199, 106, 46, 182, 179, 53, 156,
            120, 173, 177, 216, 19, 180, 28, 186, 179, 250, 233, 84, 244, 177,
        ],
        &[
            187, 194, 48, 1, 73, 36, 158, 28, 204, 132, 165, 67, 98, 35, 108, 60, 187, 194, 204,
            47, 251, 211, 182, 234,
        ],
    )
}

const SERVER_PORT: u16 = 13632;

fn logger(level: slog::Level) -> slog::Logger {
    let drain = Arc::new(
        slog_async::Async::new(
            slog_term::FullFormat::new(slog_term::TermDecorator::new().build())
                .build()
                .fuse(),
        )
            .chan_size(32768)
            .overflow_strategy(slog_async::OverflowStrategy::Block)
            .build(),
    );

    slog::Logger::root(drain.filter_level(level).fuse(), slog::o!())
}

fn build_tezedge_state() -> TezedgeState {
    // println!("generating identity...");
    // let node_identity = Identity::generate(ProofOfWork::DEFAULT_TARGET).unwrap();
    // dbg!(&node_identity);
    // dbg!(node_identity.secret_key.as_ref().0);

    let node_identity = identity_1();

    // println!("identity generated!");
    let mut tezedge_state = TezedgeState::new(
        logger(slog::Level::Trace),
        TezedgeConfig {
            port: SERVER_PORT,
            disable_mempool: true,
            private_node: false,
            disable_quotas: false,
            min_connected_peers: 500,
            max_connected_peers: 1000,
            max_pending_peers: 1000,
            max_potential_peers: 100000,
            periodic_react_interval: Duration::from_millis(250),
            reset_quotas_interval: Duration::from_secs(5),
            peer_blacklist_duration: Duration::from_secs(30 * 60),
            peer_timeout: Duration::from_secs(8),
            pow_target: ProofOfWork::DEFAULT_TARGET,
        },
        node_identity.clone(),
        shell_compatibility_version(),
        Default::default(),
        Instant::now(),
    );

    let peer_addresses = HashSet::<_>::from_iter(
        [
            // Potential peers which state machine will try to connect to.
            vec![
                "138.201.74.177:9732",
            ]
                .into_iter()
                .map(|x| x.parse().unwrap())
                .collect::<Vec<_>>(),
        ]
            .concat()
            .into_iter(),
    );

    let _ = tezedge_state.accept(ExtendPotentialPeersProposal {
        at: Instant::now(),
        peers: peer_addresses,
    });

    tezedge_state
}
type Timestamp = u32;
#[derive(Debug)]
struct P2PRequestLatency {
    sent : Timestamp,
    recv : Timestamp
}

impl P2PRequestLatency {
    fn new() -> Self {
        Self {
            sent: chrono::Utc::now().timestamp_subsec_nanos(),
            recv: 0
        }
    }

    fn duration(&self) -> Timestamp {
        self.recv - self.sent
    }
}

struct ChainSyncState {
    highest_available_block: Option<BlockHeader>,
    current_head : Option<BlockHeader>,
    peers : HashMap<IpAddr, PeerAddress>,
    block_storage : BlockStorage,
    block_meta_storage : BlockMetaStorage,
    highest_available_history: VecDeque<BlockHash>,
    stored_block_header_level: Level,
    block_headers_count: u32,
    progress : Level,
    cursor : Option<BlockHash>,
    end : Option<BlockHash>,
    start : Option<BlockHash>,
    active_peer : Option<PeerAddress>,
    block_p2p_requests_latencies : Vec<P2PRequestLatency>,
    ///Change later
    last_peer_message : Option<PeerMessage>
}

fn main() {

    let backend = NotusDBBackend::new("/tmp/tezedge/metrics/database").map(|db|{
        TezedgeDatabaseBackendOptions::Notus(db)
    }).unwrap();

    let maindb = Arc::new(TezedgeDatabase::new(backend));
    // commit log storage
    let clog = open_cl("/tmp/tezedge/metrics/block-storage", vec![BlockStorage::descriptor()]).unwrap();

    let persistent_storage = PersistentStorage::new(
        maindb.clone(),
        Arc::new(clog),
        Arc::new(Sequences::new(maindb, 1000)),
    );

    let mut chain_state = ChainSyncState {
        highest_available_block: None,
        current_head: None,
        peers: Default::default(),
        block_storage: BlockStorage::new(&persistent_storage),
        block_meta_storage: BlockMetaStorage::new(&persistent_storage),
        highest_available_history: Default::default(),
        stored_block_header_level: 0,
        block_headers_count: 0,
        progress: 0,
        cursor: None,
        end: None,
        start: None,
        active_peer: None,
        last_peer_message: None,
        block_p2p_requests_latencies: vec![]
    };

    let mut proposer = TezedgeProposer::new(
        TezedgeProposerConfig {
            wait_for_events_timeout: Some(Duration::from_millis(250)),
            events_limit: 1024,
        },
        build_tezedge_state(),
        // capacity is changed by events_limit.
        MioEvents::new(),
        MioManager::new(SERVER_PORT),
    );

    let mut counter = 3;

    let tezos_env = if let Some(tezos_network_config) = environment::default_networks().get(&TezosEnvironment::Mainnet) {
        tezos_network_config.clone()
    } else {
        panic!(
            "Missing default configuration for selected network",
        )
    };


    loop {
        //println!("{:#?}", chain_state.block_p2p_requests_latencies);
        proposer.make_progress();
        for n in proposer.take_notifications().collect::<Vec<_>>() {
            match n {
                Notification::HandshakeSuccessful { peer_address, .. } => {
                    // Send Bootstrap message.
                    proposer.send_message_to_peer_or_queue(
                        Instant::now(),
                        peer_address,
                        PeerMessage::Bootstrap,
                    );
                }
                Notification::MessageReceived { peer, message } => {

                    match &message.message {
                        PeerMessage::Disconnect => {}
                        PeerMessage::Advertise(_) => {}
                        PeerMessage::SwapRequest(_) => {}
                        PeerMessage::SwapAck(_) => {}
                        PeerMessage::Bootstrap => {

                        }
                        PeerMessage::GetCurrentBranch(_) => {
                            /*let genesis_block = tezos_env
                                .genesis_header(genesis_context_hash().try_into().unwrap(), get_empty_operation_list_list_hash().unwrap()).unwrap();
                            let chain_id = tezos_env.main_chain_id().unwrap();
                            let msg = CurrentBranchMessage::new(
                                chain_id,
                                CurrentBranch::new(genesis_block, vec![]),
                            );
                            proposer.send_message_to_peer_or_queue(Instant::now(), peer, PeerMessage::CurrentBranch(msg));
                            proposer.send_message_to_peer_or_queue(Instant::now(), peer, PeerMessage::GetCurrentBranch(GetCurrentBranchMessage::new(tezos_env.main_chain_id().unwrap())));*/
                        }
                        PeerMessage::CurrentBranch(message) => {
                            //let msg = GetCurrentHeadMessage::new(tezos_env.main_chain_id().unwrap());
                            //proposer.send_message_to_peer_or_queue(Instant::now(), peer,PeerMessage::GetCurrentHead(msg));
                            //chain_state.block_p2p_requests_latencies.push(P2PRequestLatency::new())
                            /*chain_state.peers.insert(peer.ip(), peer.clone());
                            let received_block_header: BlockHeader = message.current_branch().current_head().clone();
                            if let Some(highest_available_block) = &mut chain_state.highest_available_block {
                                if highest_available_block.level < received_block_header.level {
                                    chain_state.highest_available_block = Some(received_block_header);
                                }
                            } else {
                                let genesis_block = tezos_env
                                    .genesis_header(genesis_context_hash().try_into().unwrap(), get_empty_operation_list_list_hash().unwrap()).unwrap();
                                chain_state.highest_available_block = Some(received_block_header.clone());
                                chain_state.cursor = Some(received_block_header.clone().predecessor);
                                let genesis_block_hash: BlockHash = genesis_block.message_hash().unwrap().try_into().unwrap();
                                let start_block_hash: BlockHash = received_block_header.clone().message_hash().unwrap().try_into().unwrap();
                                chain_state.end = Some(genesis_block_hash);
                                chain_state.start = Some(start_block_hash);
                                chain_state.progress = received_block_header.level;
                                chain_state.stored_block_header_level = genesis_block.level;
                                //Send Get Block header
                                let msg = GetCurrentHeadMessage::new(tezos_env.main_chain_id().unwrap());
                                proposer.send_message_to_peer_or_queue(Instant::now(), peer,PeerMessage::GetCurrentHead(msg));
                                chain_state.block_p2p_requests_latencies.push(P2PRequestLatency::new())
                            }*/
                        }
                        PeerMessage::Deactivate(_) => {}
                        PeerMessage::GetCurrentHead(message) => {
                            println!("GetCurrentHead {:#?}", message)
                        }
                        PeerMessage::CurrentHead(message) => {
                            println!("CurrentHead {:#?}", message.current_block_header().message_hash());
                            //Loop GetCurrent head
                            //let msg = GetCurrentHeadMessage::new(tezos_env.main_chain_id().unwrap());
                            //proposer.send_message_to_peer_or_queue(Instant::now(), peer,PeerMessage::GetCurrentHead(msg));
                            //chain_state.block_p2p_requests_latencies.push(P2PRequestLatency::new())
                        }
                        PeerMessage::GetBlockHeaders(_) => {}
                        PeerMessage::BlockHeader(message) => {
                            /*if let Some(last_req) = chain_state.block_p2p_requests_latencies.last_mut() {
                                last_req.recv = chrono::Utc::now().timestamp_subsec_nanos()
                            }
                            let block_header : &BlockHeader = message.block_header();
                            chain_state.block_storage.put_block_header(&BlockHeaderWithHash::new(block_header.clone()).unwrap()).unwrap();
                            chain_state.cursor = Some(block_header.predecessor.clone());
                            let msg = GetBlockHeadersMessage::new([chain_state.cursor.clone().unwrap()].to_vec());
                            chain_state.active_peer = Some(peer);
                            chain_state.last_peer_message = Some(PeerMessage::GetBlockHeaders(msg));
                            proposer.send_message_to_peer_or_queue(Instant::now(), chain_state.active_peer.clone().unwrap(),chain_state.last_peer_message.clone().unwrap());
                            chain_state.block_p2p_requests_latencies.push(P2PRequestLatency::new());*/

                        }
                        PeerMessage::GetOperations(_) => {}
                        PeerMessage::Operation(_) => {}
                        PeerMessage::GetProtocols(_) => {}
                        PeerMessage::Protocol(_) => {}
                        PeerMessage::GetOperationsForBlocks(_) => {}
                        PeerMessage::OperationsForBlocks(_) => {}
                    }
                }
                Notification::PeerDisconnected {peer} => {
                    chain_state.peers.remove(&peer.ip());
                    match chain_state.active_peer {
                        None => {}
                        Some(active_peer) => {
                            if active_peer.to_string() == peer.to_string() {
                                //resend last message to all peers

                            }
                        }
                    }
                }
                _ => {}
            }
        }

    }
}

pub fn genesis_context_hash() -> Vec<u8> {
    let mut context_hash: Vec<u8> = Vec::new();
    context_hash.extend([14, 87, 81, 192, 38, 229, 67, 178, 232, 171, 46, 176, 96, 153, 218, 161, 209, 229, 223, 71, 119, 143, 119, 135, 250, 171, 69, 205, 241, 47, 227, 168].iter().copied());
    context_hash
}