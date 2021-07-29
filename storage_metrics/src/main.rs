use slog::Drain;
use std::collections::{HashSet, HashMap, VecDeque};
use std::iter::FromIterator;
use std::sync::{Arc, RwLock};
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
use tezos_messages::p2p::encoding::peer::PeerMessage::{GetCurrentHead, GetBlockHeaders};
use std::ops::Add;
use std::sync::atomic::{AtomicU64, Ordering, AtomicUsize, AtomicBool};
use std::str::FromStr;
use std::option::Option::Some;

const CHAIN_NAME: &'static str = "TEZOS_MAINNET";
const LOCAL_PEER: &'static str = "0.0.0.0:9732";

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
        logger(slog::Level::Error),
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
            peer_timeout: Duration::from_millis(200),
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
                LOCAL_PEER,
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
    sent: Timestamp,
    recv: Timestamp,
}

impl P2PRequestLatency {
    fn new() -> Self {
        Self {
            sent: chrono::Utc::now().timestamp_subsec_nanos(),
            recv: 0,
        }
    }

    fn duration(&self) -> Timestamp {
        self.recv - self.sent
    }
}

fn main() {
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

    let mut instance: Instant = Instant::now();
    let maximum_latency = Arc::new(AtomicUsize::new(0));
    let minimum_latency = Arc::new(AtomicUsize::new(0));
    let accumulator = Arc::new(AtomicUsize::new(0));
    let requests = Arc::new(AtomicUsize::new(0));

    let peer: PeerAddress = LOCAL_PEER.parse().unwrap();

    proposer.make_progress();

    let mut received_first_response = false;
    let mut block_header_with_hash: Option<BlockHeaderWithHash> = None;

    let mut timer: Option<Instant> = None;


    loop {
        if let Some(timer) = timer {
            if timer.elapsed().as_secs() >= 60 {

                //Print result after 60 secs
                let connected_peers = proposer.state.connected_peers();
                for connected_peer in connected_peers.iter() {
                    println!("Get Blockheaders : Peer {}", connected_peer.address);
                    if let Some(latency) = connected_peer.latencies.get("BlockHeader") {
                        println!("Total Requests: {}", latency.request_count);
                        println!("Total Requests Per Sec: {}", latency.request_count / timer.elapsed().as_secs() as u128);
                        println!("Avg Latency: {}", latency.avg_latency);
                        println!("Max Latency: {}", latency.max_latency);
                        println!("Min Latency: {}", latency.min_latency);
                    }
                }

                break;
            }
        }


        proposer.make_progress();

        /*let msg = GetCurrentHeadMessage::new(tezos_env.main_chain_id().unwrap());
        proposer.send_message_to_peer_or_queue(Instant::now(), peer,PeerMessage::GetCurrentHead(msg));
        */
        //instance = Instant::now();

        for n in proposer.take_notifications().collect::<Vec<_>>() {
            match n {
                Notification::HandshakeSuccessful { peer_address, .. } => {
                    // Send Bootstrap message.

                    //let msg = GetCurrentHeadMessage::new(tezos_env.main_chain_id().unwrap());
                    //proposer.send_message_to_peer_or_queue(Instant::now(), peer, PeerMessage::GetCurrentHead(msg));
                }
                Notification::MessageReceived { peer, message } => {
                    match &message.message {
                        PeerMessage::Disconnect => {}
                        PeerMessage::Advertise(_) => {}
                        PeerMessage::SwapRequest(_) => {}
                        PeerMessage::SwapAck(_) => {}
                        PeerMessage::Bootstrap => {}
                        PeerMessage::GetCurrentBranch(_) => {}
                        PeerMessage::CurrentBranch(message) => {}
                        PeerMessage::Deactivate(_) => {}
                        PeerMessage::GetCurrentHead(message) => {}
                        PeerMessage::CurrentHead(message) => {
                            if block_header_with_hash.is_none() {
                                block_header_with_hash = Some(BlockHeaderWithHash::new(message.current_block_header().clone()).unwrap());
                            }

                            if !received_first_response {
                                if let Some(block_header) = &block_header_with_hash {
                                    let block: BlockHash = block_header.hash.clone();
                                    let msg = GetBlockHeadersMessage::new(vec![block]);
                                    proposer.send_message_to_peer_or_queue(Instant::now(), peer, PeerMessage::GetBlockHeaders(msg));
                                    //Start timer after sending GetBlockHeaders
                                    timer = Some(Instant::now());
                                }
                            }
                            //Loop GetCurrent head
                            /*let req_latency = instance.elapsed().as_millis() as usize;
                            accumulator.fetch_add(req_latency, Ordering::Relaxed);
                            requests.fetch_add(1, Ordering::Relaxed);

                            maximum_latency.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |prev|{
                                if req_latency > prev {
                                    return Some(req_latency);
                                }
                                None
                            });

                            minimum_latency.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |prev|{
                                if req_latency == 0 {
                                    return None;
                                }
                                if req_latency < prev || prev == 0 {
                                    return Some(req_latency);
                                }
                                None
                            });


                            let msg = GetCurrentHeadMessage::new(tezos_env.main_chain_id().unwrap());
                            proposer.send_message_to_peer_or_queue(Instant::now(), peer,PeerMessage::GetCurrentHead(msg));
                            instance = Instant::now();*/
                        }
                        PeerMessage::GetBlockHeaders(_) => {}
                        PeerMessage::BlockHeader(message) => {
                            received_first_response = true;

                            if let Some(block_header) = &block_header_with_hash {
                                let msg = GetBlockHeadersMessage::new(vec![block_header.hash.clone()]);
                                proposer.send_message_to_peer_or_queue(Instant::now(), peer, PeerMessage::GetBlockHeaders(msg))
                            }
                        }
                        PeerMessage::GetOperations(_) => {}
                        PeerMessage::Operation(_) => {}
                        PeerMessage::GetProtocols(_) => {}
                        PeerMessage::Protocol(_) => {}
                        PeerMessage::GetOperationsForBlocks(_) => {}
                        PeerMessage::OperationsForBlocks(_) => {}
                    }
                }
                Notification::PeerDisconnected { peer } => {}
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