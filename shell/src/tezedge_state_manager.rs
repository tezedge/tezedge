// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Module takes care about (initialization, thread starting, ...) one singletons [TezedgeState] and [TezedgeProposer]

use std::collections::HashSet;
use std::iter::FromIterator;
use std::net::{IpAddr, SocketAddr};
use std::sync::mpsc::Receiver;
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant, SystemTime};

use dns_lookup::LookupError;
use failure::Fail;
use rand::{rngs::StdRng, Rng, SeedableRng as _};
use riker::actors::*;
use slog::{info, warn, Logger};

use crypto::hash::ChainId;
use networking::p2p::network_channel::{
    NetworkChannelMsg, NetworkChannelRef, NetworkChannelTopic, PeerMessageReceived,
};
use networking::{LocalPeerInfo, PeerId, ShellCompatibilityVersion};
use tezos_identity::Identity;

use crate::PeerConnectionThreshold;

use tla_sm::Acceptor;

use tezedge_state::proposals::ExtendPotentialPeersProposal;
use tezedge_state::{Effects, TezedgeConfig, TezedgeState};

use tezedge_state::proposer::mio_manager::{MioEvents, MioManager};
use tezedge_state::proposer::{Notification, TezedgeProposer, TezedgeProposerConfig};

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

    /// Effects seed
    pub effects_seed: Option<u64>,
}

impl P2p {
    pub const DEFAULT_P2P_PORT_FOR_LOOKUP: u16 = 9732;
}

pub enum ProposerMsg {
    NetworkChannel(NetworkChannelMsg),
}

impl From<NetworkChannelMsg> for ProposerMsg {
    fn from(msg: NetworkChannelMsg) -> Self {
        Self::NetworkChannel(msg)
    }
}

#[derive(Debug, Fail)]
pub enum NotifyProposerError {
    #[fail(display = "Notify proposer failed, reason: {:?}", _0)]
    IO(std::io::Error),
    #[fail(display = "Notify proposer failed (queue), reason: {:?}", _0)]
    SendError(std::sync::mpsc::SendError<ProposerMsg>),
}

impl From<std::io::Error> for NotifyProposerError {
    fn from(err: std::io::Error) -> Self {
        Self::IO(err)
    }
}

impl From<std::sync::mpsc::SendError<ProposerMsg>> for NotifyProposerError {
    fn from(err: std::sync::mpsc::SendError<ProposerMsg>) -> Self {
        Self::SendError(err)
    }
}

/// Handle for [TezedgeProposer].
///
/// Using this, messages can be sent to state machine thread using mpsc
/// channel and a waker for state machine thread.
#[derive(Clone)]
pub struct ProposerHandle {
    waker: Arc<mio::Waker>,
    sender: mpsc::SyncSender<ProposerMsg>,
}

impl ProposerHandle {
    pub fn new(waker: Arc<mio::Waker>, sender: mpsc::SyncSender<ProposerMsg>) -> Self {
        Self { waker, sender }
    }

    /// Send message to state machine thread.
    ///
    /// Sends the message and wakes up state machine thread.
    pub fn notify<T>(&self, msg: T) -> Result<(), NotifyProposerError>
    where
        T: Into<ProposerMsg>,
    {
        self.sender.send(msg.into())?;
        // wake will cause [TezedgeProposer::wait_for_events] ->
        // [TezedgeProposer::make_progress] to wake up and stop blocking.
        self.waker.wake()?;
        Ok(())
    }
}

fn run<Efs: Effects>(
    mut proposer: TezedgeProposer<MioEvents, Efs, MioManager>,
    rx: mpsc::Receiver<ProposerMsg>,
    network_channel: NetworkChannelRef,
    _log: &Logger,
) {
    loop {
        proposer.make_progress();

        // take notifications from state machine and send them on network
        // channel, to notify other actors about events.
        for notification in proposer.take_notifications() {
            match notification {
                Notification::HandshakeSuccessful {
                    peer_address,
                    peer_public_key_hash,
                    metadata,
                    network_version,
                } => {
                    network_channel.tell(
                        Publish {
                            msg: NetworkChannelMsg::PeerBootstrapped(
                                Arc::new(PeerId {
                                    address: peer_address,
                                    public_key_hash: peer_public_key_hash,
                                }),
                                metadata,
                                Arc::new(network_version),
                            ),
                            topic: NetworkChannelTopic::NetworkEvents.into(),
                        },
                        None,
                    );
                }
                Notification::MessageReceived { peer, message } => {
                    network_channel.tell(
                        Publish {
                            msg: NetworkChannelMsg::PeerMessageReceived(PeerMessageReceived {
                                peer_address: peer,
                                message,
                            }),
                            topic: NetworkChannelTopic::NetworkEvents.into(),
                        },
                        None,
                    );
                }
                Notification::PeerDisconnected { peer } => {
                    network_channel.tell(
                        Publish {
                            // TODO: probably this should be separate Disconnect msg.
                            msg: NetworkChannelMsg::PeerDisconnected(peer),
                            topic: NetworkChannelTopic::NetworkEvents.into(),
                        },
                        None,
                    );
                }
                Notification::PeerBlacklisted { peer } => {
                    network_channel.tell(
                        Publish {
                            msg: NetworkChannelMsg::PeerBlacklisted(peer),
                            topic: NetworkChannelTopic::NetworkEvents.into(),
                        },
                        None,
                    );
                }
            }
        }

        // Read and handle messages incoming from actor system or `PeerManager`.
        loop {
            match rx.try_recv() {
                Ok(ProposerMsg::NetworkChannel(msg)) => match msg {
                    NetworkChannelMsg::PeerStalled(peer_id) => {
                        proposer.disconnect_peer(Instant::now(), peer_id.address);
                    }
                    NetworkChannelMsg::BlacklistPeer(peer_id, reason) => {
                        proposer.blacklist_peer(Instant::now(), peer_id.address);
                    }
                    NetworkChannelMsg::SendMessage(peer_id, message) => {
                        proposer.enqueue_send_message_to_peer(
                            Instant::now(),
                            peer_id.address,
                            message.message.clone(),
                        );
                    }
                    _ => (),
                },
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => return,
            }
        }
    }
}

/// Do DNS lookup for collection of names and create collection of socket addresses
fn dns_lookup_peers(
    bootstrap_addresses: &HashSet<(String, u16)>,
    log: &Logger,
) -> HashSet<SocketAddr> {
    let mut resolved_peers = HashSet::new();
    for (address, port) in bootstrap_addresses {
        match resolve_dns_name_to_peer_address(address, *port) {
            Ok(peers) => resolved_peers.extend(&peers),
            Err(e) => {
                warn!(log, "DNS lookup failed"; "address" => address, "reason" => format!("{:?}", e))
            }
        }
    }
    resolved_peers
}

/// Try to resolve common peer name into Socket Address representation
fn resolve_dns_name_to_peer_address(
    address: &str,
    port: u16,
) -> Result<Vec<SocketAddr>, LookupError> {
    // filter just for [`AI_SOCKTYPE SOCK_STREAM`]
    let hints = dns_lookup::AddrInfoHints {
        socktype: i32::from(dns_lookup::SockType::Stream),
        ..dns_lookup::AddrInfoHints::default()
    };

    let addrs =
        dns_lookup::getaddrinfo(Some(address), Some(port.to_string().as_str()), Some(hints))?
            .filter(Result::is_ok)
            .map(Result::unwrap)
            .filter(|info: &dns_lookup::AddrInfo| {
                // filter just IP_NET and IP_NET6 addresses
                dns_lookup::AddrFamily::Inet.eq(&info.address)
                    || dns_lookup::AddrFamily::Inet6.eq(&info.address)
            })
            .map(|info: dns_lookup::AddrInfo| {
                // convert to uniform IPv6 format
                match &info.sockaddr {
                    SocketAddr::V4(ipv4) => {
                        // convert ipv4 to ipv6
                        SocketAddr::new(IpAddr::V6(ipv4.ip().to_ipv6_mapped()), ipv4.port())
                    }
                    SocketAddr::V6(_) => info.sockaddr,
                }
            })
            .collect();
    Ok(addrs)
}

enum ProposerThreadHandle {
    Running(std::thread::JoinHandle<()>),
    NotRunning(
        TezedgeState,
        MioManager,
        StdRng,
        Receiver<ProposerMsg>,
        NetworkChannelRef,
        HashSet<SocketAddr>,
    ),
}

pub struct TezedgeStateManager {
    proposer_handle: ProposerHandle,
    proposer_thread_handle: Option<ProposerThreadHandle>,
    log: Logger,
}

impl TezedgeStateManager {
    const PROPOSER_QUEUE_MAX_CAPACITY: usize = 100_000;

    pub fn new(
        network_channel: NetworkChannelRef,
        log: Logger,
        identity: Arc<Identity>,
        shell_compatibility_version: Arc<ShellCompatibilityVersion>,
        p2p_config: P2p,
        pow_target: f64,
        chain_id: ChainId,
    ) -> Self {
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
        let listener_port = p2p_config.listener_port;

        let local_node_info = Arc::new(LocalPeerInfo::new(
            listener_port,
            identity,
            shell_compatibility_version,
            pow_target,
        ));

        let (proposer_tx, proposer_rx) = mpsc::sync_channel(Self::PROPOSER_QUEUE_MAX_CAPACITY);

        // override port passed listener address
        let mut listener_addr = p2p_config.listener_address;
        listener_addr.set_port(listener_port);

        let mio_manager = MioManager::new(listener_addr);
        let proposer_handle = ProposerHandle::new(mio_manager.waker(), proposer_tx);

        let seed = p2p_config.effects_seed.unwrap_or_else(|| {
            let seed = rand::thread_rng().gen();
            info!(log, "Proposer's effects seed selected"; "seed" => seed);
            seed
        });
        let mut effects = StdRng::seed_from_u64(seed);

        let mut tezedge_state = TezedgeState::new(
            log.clone(),
            TezedgeConfig {
                port: p2p_config.listener_port,
                disable_mempool: p2p_config.disable_mempool,
                private_node: p2p_config.private_node,
                // TODO: TE-652 - disable_quotas from cfg or env
                disable_quotas: false,
                disable_blacklist: p2p_config.disable_blacklist,
                min_connected_peers: p2p_config.peer_threshold.low,
                max_connected_peers: p2p_config.peer_threshold.high,
                max_pending_peers: p2p_config.peer_threshold.high,
                max_potential_peers: p2p_config.peer_threshold.high * 20,
                periodic_react_interval: Duration::from_millis(250),
                reset_quotas_interval: Duration::from_secs(5),
                peer_blacklist_duration: Duration::from_secs(8 * 60),
                peer_timeout: Duration::from_secs(8),
                pow_target: local_node_info.pow_target(),
            },
            (*local_node_info.identity()).clone(),
            (*local_node_info.version()).clone(),
            &mut effects,
            // TODO: this is temporary until snapshot of initial state
            // is available. Should be `SystemTime::now()` to set
            // state machine's to actual clock. Not mandatory or critical,
            // just useful to have. We have to do this for record/replay
            // functionality, so that initial time for record/replay is same.
            SystemTime::UNIX_EPOCH,
            chain_id,
        );

        info!(log, "Doing peer DNS lookup"; "bootstrap_addresses" => format!("{:?}", &bootstrap_addresses));
        let initial_potential_peers = dbg!(dns_lookup_peers(&bootstrap_addresses, &log));

        Self {
            proposer_handle,
            log,
            proposer_thread_handle: Some(ProposerThreadHandle::NotRunning(
                tezedge_state,
                mio_manager,
                effects,
                proposer_rx,
                network_channel,
                initial_potential_peers,
            )),
        }
    }

    pub fn start(&mut self) {
        if let Some(ProposerThreadHandle::NotRunning(
            tezedge_state,
            mio_manager,
            effects,
            proposer_rx,
            network_channel,
            initial_potential_peers,
        )) = self.proposer_thread_handle.take()
        {
            let log = self.log.clone();

            let mut proposer = TezedgeProposer::new(
                Instant::now(),
                log.clone(),
                TezedgeProposerConfig {
                    wait_for_events_timeout: Some(Duration::from_millis(250)),
                    events_limit: 1024,
                    // TODO: add cli argument
                    record: std::env::var("RECORD_PROPOSALS")
                        .map(|x| x == "1")
                        .unwrap_or(false),
                    replay: std::env::var("REPLAY_PROPOSALS")
                        .map(|x| x == "1")
                        .unwrap_or(false),
                },
                effects,
                tezedge_state,
                MioEvents::new(),
                mio_manager,
            );

            // start to listen for incoming p2p connections and state machine processing
            let proposer_thread_handle = std::thread::Builder::new()
                .name("tezedge-proposer".to_owned())
                .spawn(move || {
                    proposer.extend_potential_peers(Instant::now(), initial_potential_peers);
                    run(proposer, proposer_rx, network_channel, &log);
                })
                .expect("failed to spawn proposer-thread");

            self.proposer_thread_handle =
                Some(ProposerThreadHandle::Running(proposer_thread_handle));
        }
    }

    pub fn proposer_handle(&self) -> ProposerHandle {
        self.proposer_handle.clone()
    }
}

impl Drop for TezedgeStateManager {
    fn drop(&mut self) {
        info!(self.log, "Closing Tezedge state manager");

        let TezedgeStateManager {
            proposer_handle,
            log,
            ..
        } = self;
        let ProposerHandle { waker, .. } = proposer_handle;

        // dropping mpsc sender will cause state machine thread
        // to exit once it tries to read from this mpsc channel.
        // drop(sender);

        if let Err(e) = waker.wake() {
            warn!(log, "Failed to shutdown proposer waker"; "reason" => e);
        }
    }
}
