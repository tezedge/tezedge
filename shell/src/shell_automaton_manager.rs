// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Module takes care about (initialization, thread starting, ...) of ShellAutomaton.

use std::collections::HashSet;
use std::env;
use std::iter::FromIterator;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use rand::{rngs::StdRng, Rng, SeedableRng as _};
use shell_automaton::service::rpc_service::RpcShellAutomatonChannel;
use slog::{info, o, warn, Logger};
use storage::PersistentStorage;

use crypto::hash::ChainId;
use networking::network_channel::NetworkChannelRef;
use tezos_identity::Identity;

use crate::PeerConnectionThreshold;

pub use shell_automaton::service::actors_service::{
    ActorsMessageFrom as ShellAutomatonMsg, AutomatonSyncSender as ShellAutomatonSender,
};
use shell_automaton::service::mio_service::MioInternalEventsContainer;
use shell_automaton::service::{
    ActorsServiceDefault, DnsServiceDefault, MioServiceDefault, RpcServiceDefault, ServiceDefault,
    StorageServiceDefault,
};
use shell_automaton::shell_compatibility_version::ShellCompatibilityVersion;
use shell_automaton::{Port, ShellAutomaton};

#[derive(Debug, Clone)]
pub struct P2p {
    /// Node p2p port
    pub listener_port: u16,
    /// P2p socket address, where node listens for incoming p2p connections
    pub listener_address: SocketAddr,

    pub disable_mempool: bool,
    pub disable_blacklist: bool,
    pub private_node: bool,

    pub peer_threshold: PeerConnectionThreshold,

    /// Bootstrap lookup addresses disable/enable
    pub disable_bootstrap_lookup: bool,
    /// Used for lookup with DEFAULT_P2P_PORT_FOR_LOOKUP
    pub bootstrap_lookup_addresses: Vec<(String, u16)>,

    /// Peers (IP:port) which we try to connect all the time
    pub bootstrap_peers: Vec<SocketAddr>,

    /// Randomness seed for [shell_automaton::ShellAutomaton].
    pub randomness_seed: Option<u64>,

    pub persist_proposals: Option<String>,
    pub replay_proposals: Option<String>,
}

impl P2p {
    pub const DEFAULT_P2P_PORT_FOR_LOOKUP: u16 = 9732;
}

enum ShellAutomatonThreadHandle {
    Running(std::thread::JoinHandle<()>),
    NotRunning(
        P2p,
        ShellAutomaton<ServiceDefault, MioInternalEventsContainer>,
        HashSet<(String, Port)>,
    ),
}

pub struct ShellAutomatonManager {
    shell_automaton_sender: ShellAutomatonSender,
    shell_automaton_thread_handle: Option<ShellAutomatonThreadHandle>,
    log: Logger,
}

impl ShellAutomatonManager {
    const SHELL_AUTOMATON_QUEUE_MAX_CAPACITY: usize = 100_000;

    pub fn new(
        persistent_storage: PersistentStorage,
        network_channel: NetworkChannelRef,
        log: Logger,
        identity: Arc<Identity>,
        shell_compatibility_version: Arc<ShellCompatibilityVersion>,
        p2p_config: P2p,
        pow_target: f64,
        _chain_id: ChainId,
    ) -> (Self, RpcShellAutomatonChannel) {
        // resolve all bootstrap addresses - init from bootstrap_peers
        let mut bootstrap_addresses = HashSet::from_iter(
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

        let mio_service = MioServiceDefault::new(listener_addr);
        let (rpc_service, rpc_channel) = RpcServiceDefault::new(mio_service.waker(), 128);

        let storage_service =
            StorageServiceDefault::init(mio_service.waker(), persistent_storage.clone(), 4096);

        let (automaton_sender, automaton_receiver) =
            shell_automaton::service::actors_service::sync_channel(
                mio_service.waker(),
                Self::SHELL_AUTOMATON_QUEUE_MAX_CAPACITY,
            );

        let quota_service = shell_automaton::service::QuotaServiceDefault::new(
            mio_service.waker(),
            Duration::from_millis(100),
            log.new(o!("service" => "quota")),
        );

        let service = ServiceDefault {
            randomness: StdRng::seed_from_u64(seed),
            dns: DnsServiceDefault::default(),
            mio: mio_service,
            storage: storage_service,
            rpc: rpc_service,
            actors: ActorsServiceDefault::new(automaton_receiver, network_channel),
            quota: quota_service,
        };

        let events = MioInternalEventsContainer::with_capacity(1024);

        let initial_state = shell_automaton::State::new(shell_automaton::Config {
            initial_time: SystemTime::now(),

            port: p2p_config.listener_port,
            disable_mempool: p2p_config.disable_mempool,
            private_node: p2p_config.private_node,
            identity: (*identity).clone(),
            shell_compatibility_version: (*shell_compatibility_version).clone(),
            pow_target,

            quota: shell_automaton::Quota {
                restore_duration_millis: env_variable("QUOTA_RESTORE_DURATION_MILLIS")
                    .unwrap_or(1000),
                read_quota: env_variable("QUOTA_READ_BYTES").unwrap_or(128 * 1024),
                write_quota: env_variable("QUOTA_WRITE_BYTES").unwrap_or(128 * 1024),
            },
        });

        let shell_automaton = ShellAutomaton::new(initial_state, service, events);

        let this = Self {
            log,
            shell_automaton_sender: automaton_sender,
            shell_automaton_thread_handle: Some(ShellAutomatonThreadHandle::NotRunning(
                p2p_config,
                shell_automaton,
                bootstrap_addresses,
            )),
        };

        (this, rpc_channel)
    }

    pub fn start(&mut self) {
        if let Some(ShellAutomatonThreadHandle::NotRunning(
            _config,
            mut shell_automaton,
            bootstrap_addresses,
        )) = self.shell_automaton_thread_handle.take()
        {
            // start to listen for incoming p2p connections and state machine processing
            let shell_automaton_thread_handle = std::thread::Builder::new()
                .name("shell-automaton".to_owned())
                .spawn(move || {
                    shell_automaton.init(bootstrap_addresses);

                    loop {
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
}

impl Drop for ShellAutomatonManager {
    fn drop(&mut self) {
        info!(self.log, "Closing Tezedge state manager");

        let ShellAutomatonManager {
            shell_automaton_sender,
            ..
        } = self;
        if let Err(err) = shell_automaton_sender.send(ShellAutomatonMsg::Shutdown) {
            warn!(self.log, "Failed to send Shutdown message to ShellAutomaton"; "error" => format!("{:?}", err));
        }
    }
}

fn env_variable<T: FromStr>(name: &str) -> Option<T> {
    env::var(name).ok().and_then(|v| v.parse().ok())
}
