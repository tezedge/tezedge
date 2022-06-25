// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Module takes care about (initialization, thread starting, ...) of ShellAutomaton.

use std::collections::HashSet;
use std::env;
use std::fs::File;
use std::iter::FromIterator;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use rand::{rngs::StdRng, Rng, SeedableRng as _};
use serde::Deserialize;
use slog::{info, warn, Logger};

use networking::network_channel::NetworkChannelRef;
use storage::{PersistentStorage, StorageInitInfo};
use tezos_identity::Identity;
use tezos_messages::base::signature_public_key::SignaturePublicKeyHash;
use tezos_messages::p2p::encoding::block_header::Level;
use tezos_protocol_ipc_client::{ProtocolRunnerApi, ProtocolRunnerConfiguration};

use shell_automaton::baker::block_baker::LiquidityBakingToggleVote;
pub use shell_automaton::service::actors_service::{
    ActorsMessageFrom as ShellAutomatonMsg, AutomatonSyncSender as ShellAutomatonSender,
};
pub use shell_automaton::service::actors_service::{ApplyBlockCallback, ApplyBlockResult};
use shell_automaton::service::baker_service::BakerSigner;
use shell_automaton::service::mio_service::MioInternalEventsContainer;
use shell_automaton::service::rpc_service::RpcShellAutomatonSender;
use shell_automaton::service::{
    ActorsServiceDefault, BakerServiceDefault, DnsServiceDefault, MioServiceDefault,
    ProtocolRunnerServiceDefault, RpcServiceDefault, ServiceDefault, StorageServiceDefault,
};
use shell_automaton::shell_compatibility_version::ShellCompatibilityVersion;
use shell_automaton::ShellAutomaton;

use crate::PeerConnectionThreshold;

#[derive(Debug, Clone)]
pub struct P2p {
    /// Node p2p port
    pub listener_port: u16,
    /// P2p socket address, where node listens for incoming p2p connections
    pub listener_address: SocketAddr,

    pub disable_mempool: bool,
    pub disable_block_precheck: bool,
    pub disable_endorsements_precheck: bool,
    pub disable_peer_graylist: bool,
    pub private_node: bool,

    pub peer_threshold: PeerConnectionThreshold,

    /// Bootstrap lookup addresses disable/enable
    pub disable_bootstrap_lookup: bool,
    /// Used for lookup with DEFAULT_P2P_PORT_FOR_LOOKUP
    pub bootstrap_lookup_addresses: Vec<(String, u16)>,

    /// Peers (IP:port) which we try to connect all the time
    pub bootstrap_peers: Vec<SocketAddr>,

    pub current_head_level_override: Option<Level>,

    /// Randomness seed for [shell_automaton::ShellAutomaton].
    pub randomness_seed: Option<u64>,

    pub record_shell_automaton_state_snapshots: bool,
    pub record_shell_automaton_actions: bool,

    pub baker_data_dir: PathBuf,
    pub baker_names: Vec<String>,
    pub liquidity_baking_escape_vote: LiquidityBakingToggleVote,
}

impl P2p {
    pub const DEFAULT_P2P_PORT_FOR_LOOKUP: u16 = 9732;
}

enum ShellAutomatonThreadHandle {
    Running(std::thread::JoinHandle<()>),
    NotRunning(Box<ShellAutomaton<ServiceDefault, MioInternalEventsContainer>>),
}

pub struct ShellAutomatonManager {
    shell_automaton_sender: ShellAutomatonSender,
    shell_automaton_thread_handle: Option<ShellAutomatonThreadHandle>,
    log: Logger,
}

impl ShellAutomatonManager {
    const SHELL_AUTOMATON_QUEUE_MAX_CAPACITY: usize = 100_000;

    pub fn new(
        protocol_runner_api: ProtocolRunnerApi,
        persistent_storage: PersistentStorage,
        network_channel: NetworkChannelRef,
        log: Logger,
        identity: Arc<Identity>,
        shell_compatibility_version: Arc<ShellCompatibilityVersion>,
        p2p_config: P2p,
        pow_target: f64,
        init_storage_data: StorageInitInfo,
        protocol_runner_config: ProtocolRunnerConfiguration,
        context_init_status_sender: tokio::sync::watch::Sender<bool>,
    ) -> (Self, RpcShellAutomatonSender) {
        // resolve all bootstrap addresses - init from bootstrap_peers
        let mut bootstrap_addresses = HashSet::<_>::from_iter(
            p2p_config
                .bootstrap_peers
                .iter()
                .map(|addr| (addr.ip().to_string(), addr.port())),
        );

        // if lookup enabled, add also configuted lookup addresses
        if !p2p_config.disable_bootstrap_lookup {
            bootstrap_addresses.extend(p2p_config.bootstrap_lookup_addresses.iter().cloned());
        };

        // override port passed listener address
        let mut listener_addr = p2p_config.listener_address;
        listener_addr.set_port(p2p_config.listener_port);

        let seed = p2p_config.randomness_seed.unwrap_or_else(|| {
            let seed = rand::thread_rng().gen();
            info!(log, "Automaton's randomness seed selected"; "seed" => seed);
            seed
        });

        let mio_service = MioServiceDefault::new(
            listener_addr,
            // Buffer size for reading. Chunk size is 2 bytes (u16) and that is
            // the max number of bytes that we will want to read from kernel at
            // any given point.
            u16::MAX as usize,
        );
        let (rpc_service, rpc_channel) = RpcServiceDefault::new(mio_service.waker(), 128);

        let storage_service =
            StorageServiceDefault::init(log.clone(), mio_service.waker(), persistent_storage, 4096);

        let (automaton_sender, automaton_receiver) =
            shell_automaton::service::actors_service::sync_channel(
                mio_service.waker(),
                Self::SHELL_AUTOMATON_QUEUE_MAX_CAPACITY,
            );

        let protocol_runner_service = ProtocolRunnerServiceDefault::new(
            protocol_runner_api,
            mio_service.waker(),
            64,
            context_init_status_sender,
            log.new(slog::o!("service" => "protocol_runner")),
        );

        let bakers_cfg = read_bakers_config(&p2p_config.baker_data_dir, &p2p_config.baker_names);
        let bakers_pkhs = bakers_cfg
            .iter()
            .map(|(_, pkh)| pkh.clone())
            .collect::<Vec<_>>();
        let mut baker_service =
            BakerServiceDefault::new(mio_service.waker(), p2p_config.baker_data_dir.clone());
        for (signer, pkh) in bakers_cfg {
            baker_service.add_signer(pkh, signer);
        }

        let service = ServiceDefault {
            randomness: StdRng::seed_from_u64(seed),
            dns: DnsServiceDefault::default(),
            mio: mio_service,
            protocol_runner: protocol_runner_service,
            storage: storage_service,
            rpc: rpc_service,
            actors: ActorsServiceDefault::new(automaton_receiver, network_channel),
            baker: baker_service,
            statistics: Some(Default::default()),
        };

        let events = MioInternalEventsContainer::with_capacity(1024);

        let chain_id = init_storage_data.chain_id.clone();

        let bakers_config = bakers_pkhs
            .into_iter()
            .map(|pkh| shell_automaton::config::BakerConfig {
                pkh,
                liquidity_baking_escape_vote: p2p_config.liquidity_baking_escape_vote,
            })
            .collect();
        let mut initial_state = shell_automaton::State::new(shell_automaton::Config {
            initial_time: SystemTime::now(),

            protocol_runner: protocol_runner_config,
            init_storage_data,

            port: p2p_config.listener_port,
            disable_mempool: p2p_config.disable_mempool,
            private_node: p2p_config.private_node,
            identity: (*identity).clone(),
            shell_compatibility_version: (*shell_compatibility_version).clone(),
            pow_target,
            chain_id,

            check_timeouts_interval: Duration::from_millis(200),

            peers_dns_lookup_addresses: bootstrap_addresses.into_iter().collect(),

            peer_connecting_timeout: Duration::from_secs(4),
            peer_handshaking_timeout: Duration::from_secs(8),

            peer_max_io_syscalls: 32,

            peers_potential_max: p2p_config.peer_threshold.high * 5,
            peers_connected_min: p2p_config.peer_threshold.low,
            peers_connected_max: p2p_config.peer_threshold.high,
            peers_bootstrapped_min: p2p_config
                .peer_threshold
                .num_of_peers_for_bootstrap_threshold(),

            peers_graylist_disable: p2p_config.disable_peer_graylist,
            peers_graylist_timeout: Duration::from_secs(15 * 60),

            bootstrap_block_header_get_timeout: Duration::from_millis(500),
            bootstrap_block_operations_get_timeout: Duration::from_millis(1000),

            current_head_level_override: p2p_config.current_head_level_override,

            record_state_snapshots_with_interval: match p2p_config
                .record_shell_automaton_state_snapshots
            {
                true => Some(10_000),
                false => None,
            },
            record_actions: p2p_config.record_shell_automaton_actions,

            disable_block_precheck: p2p_config.disable_block_precheck,
            disable_endorsements_precheck: p2p_config.disable_endorsements_precheck,
            mempool_get_operation_timeout: Duration::from_millis(
                env_variable("MEMPOOL_GET_OPERATIONS_TIMEOUT_SECS").unwrap_or(1),
            ),

            bakers: bakers_config,
            liquidity_baking_escape_vote: p2p_config.liquidity_baking_escape_vote,
        });

        initial_state.set_logger(log.clone());

        let shell_automaton = ShellAutomaton::new(initial_state, service, events);

        let this = Self {
            log,
            shell_automaton_sender: automaton_sender,
            shell_automaton_thread_handle: Some(ShellAutomatonThreadHandle::NotRunning(Box::new(
                shell_automaton,
            ))),
        };

        (this, rpc_channel)
    }

    pub fn start(&mut self) {
        if let Some(ShellAutomatonThreadHandle::NotRunning(mut shell_automaton)) =
            self.shell_automaton_thread_handle.take()
        {
            // start to listen for incoming p2p connections and state machine processing
            let shell_automaton_thread_handle = std::thread::Builder::new()
                .name("shell-automaton".to_owned())
                .spawn(move || {
                    shell_automaton.init();

                    while !shell_automaton.is_shutdown() {
                        shell_automaton.make_progress();
                    }
                })
                .expect("failed to spawn shell-automaton-thread");

            self.shell_automaton_thread_handle = Some(ShellAutomatonThreadHandle::Running(
                shell_automaton_thread_handle,
            ));
        }
    }

    pub fn shell_automaton_sender(&self) -> ShellAutomatonSender {
        self.shell_automaton_sender.clone()
    }

    pub fn send_shutdown_signal(&self) {
        if let Err(err) = self
            .shell_automaton_sender
            .send(ShellAutomatonMsg::Shutdown)
        {
            warn!(self.log, "Failed to send Shutdown message to ShellAutomaton"; "error" => format!("{:?}", err));
        }
    }

    pub fn shutdown_and_wait(self) {
        self.send_shutdown_signal();

        if let Some(ShellAutomatonThreadHandle::Running(th)) = self.shell_automaton_thread_handle {
            th.join().unwrap();
        }
    }
}

fn env_variable<T: FromStr>(name: &str) -> Option<T> {
    env::var(name).ok().and_then(|v| v.parse().ok())
}

pub fn read_bakers_config(
    base_dir: &PathBuf,
    baker_names: &[String],
) -> Vec<(BakerSigner, SignaturePublicKeyHash)> {
    #[derive(Deserialize)]
    struct SecretKeyRecord {
        name: String,
        value: String,
    }

    if baker_names.is_empty() {
        return vec![];
    }

    let secret_keys = File::open(base_dir.join("secret_keys")).expect(&format!(
        "Failed to read baker 'secret_keys' file {:?}",
        base_dir.join("secret_keys")
    ));
    let secret_keys = serde_json::from_reader::<_, Vec<SecretKeyRecord>>(secret_keys)
        .expect("Failed to parse baker 'secret_keys' file");
    secret_keys
        .iter()
        .filter(|v| baker_names.contains(&v.name))
        .map(|v| {
            BakerSigner::parse_config_str(&v.value)
                .expect("Failed to parse baker's value in 'secret_keys' file")
        })
        .collect()
}
