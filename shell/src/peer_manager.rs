// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Manages connected peers.

use std::cmp;
use std::collections::{HashMap, HashSet};
use std::iter::FromIterator;
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, PoisonError, RwLock, Weak};
use std::time::{Duration, Instant};

use dns_lookup::LookupError;
use rand::seq::SliceRandom;
use riker::actors::*;
use slog::{crit, debug, error, info, trace, warn, Logger};
use thiserror::Error;
use tokio::net::{TcpListener, TcpStream};
use tokio::runtime::Handle;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::time::timeout;

use networking::p2p::network_channel::{
    NetworkChannelMsg, NetworkChannelRef, NetworkChannelTopic, PeerMessageReceived,
};
use networking::{LocalPeerInfo, PeerId, ShellCompatibilityVersion};
use tezos_identity::Identity;
use tezos_messages::p2p::binary_message::{BinaryRead, BinaryWrite};
use tezos_messages::p2p::encoding::limits::ADVERTISE_ID_LIST_MAX_LENGTH_FOR_SEND;
use tezos_messages::p2p::encoding::prelude::*;

use crate::shell_channel::{ShellChannelMsg, ShellChannelRef};
use crate::subscription::*;
use crate::PeerConnectionThreshold;

use tla_sm::Acceptor;

use tezedge_state::proposals::ExtendPotentialPeersProposal;
use tezedge_state::{DefaultEffects, TezedgeConfig, TezedgeState};

use tezedge_state::proposer::mio_manager::{MioEvents, MioManager};
use tezedge_state::proposer::{Notification, TezedgeProposer, TezedgeProposerConfig};

/// Message commands [`PeerManager`] to log its internal stats.
#[derive(Clone, Debug)]
pub struct LogPeerStats;

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
}

impl P2p {
    pub const DEFAULT_P2P_PORT_FOR_LOOKUP: u16 = 9732;
}

/// Possible errors for state processing
#[derive(Debug, Error)]
pub enum PeerManagerError {
    #[error("Mutex/lock error, reason: {reason:?}")]
    LockError { reason: String },
}

impl<T> From<PoisonError<T>> for PeerManagerError {
    fn from(pe: PoisonError<T>) -> Self {
        PeerManagerError::LockError {
            reason: format!("{}", pe),
        }
    }
}

enum ProposerMsg {
    NetworkChannel(NetworkChannelMsg),
}

impl From<NetworkChannelMsg> for ProposerMsg {
    fn from(msg: NetworkChannelMsg) -> Self {
        Self::NetworkChannel(msg)
    }
}

#[derive(Debug, Error)]
enum NotifyProposerError {
    #[error(display = "notify proposer failed, reason: {0}")]
    IO(std::io::Error),
}

impl From<std::io::Error> for NotifyProposerError {
    fn from(err: std::io::Error) -> Self {
        Self::IO(err)
    }
}

struct ProposerHandle {
    waker: Arc<mio::Waker>,
    sender: mpsc::Sender<ProposerMsg>,
}

impl ProposerHandle {
    pub fn new(waker: Arc<mio::Waker>, sender: mpsc::Sender<ProposerMsg>) -> Self {
        Self { waker, sender }
    }

    pub fn notify<T>(&self, msg: T) -> Result<(), NotifyProposerError>
    where
        T: Into<ProposerMsg>,
    {
        self.sender.send(msg.into());
        self.waker.wake()?;
        Ok(())
    }
}

/// This actor is responsible for peer management.
///
/// It monitors number of connected peers. If the number of connected peers is too low it tries to
/// connect to more peers. If the number of connected peers is too high, then randomly selected peers
/// are disconnected.
#[actor(NetworkChannelMsg, ShellChannelMsg)]
pub struct PeerManager {
    /// All events generated by the network layer will end up in this channel
    network_channel: NetworkChannelRef,
    /// All events from shell will be published to this channel
    shell_channel: ShellChannelRef,
    /// Tokio runtime
    tokio_executor: Handle,

    config: P2p,
    proposer: Option<ProposerHandle>,

    /// Bootstrap peer, which we try to connect all the the, if no other peers presents
    bootstrap_addresses: HashSet<(String, u16)>,

    /// Local node info covers:
    /// - listener_port - we will listen for incoming connection at this port
    /// - identity
    /// - Network/protocol version
    local_node_info: Arc<LocalPeerInfo>,
    /// Last time we did DNS peer discovery
    discovery_last: Option<Instant>,
}

/// Reference to [peer manager](PeerManager) actor.
pub type PeerManagerRef = ActorRef<PeerManagerMsg>;

impl PeerManager {
    pub fn actor(
        sys: &impl ActorRefFactory,
        network_channel: NetworkChannelRef,
        shell_channel: ShellChannelRef,
        tokio_executor: Handle,
        identity: Arc<Identity>,
        shell_compatibility_version: Arc<ShellCompatibilityVersion>,
        p2p_config: P2p,
        pow_target: f64,
    ) -> Result<PeerManagerRef, CreateError> {
        sys.actor_of_props::<PeerManager>(
            PeerManager::name(),
            Props::new_args((
                network_channel,
                shell_channel,
                tokio_executor,
                identity,
                shell_compatibility_version,
                p2p_config,
                pow_target,
            )),
        )
    }

    /// The `PeerManager` is intended to serve as a singleton actor so that's why
    /// we won't support multiple names per instance.
    fn name() -> &'static str {
        "peer-manager"
    }
}

impl
    ActorFactoryArgs<(
        NetworkChannelRef,
        ShellChannelRef,
        Handle,
        Arc<Identity>,
        Arc<ShellCompatibilityVersion>,
        P2p,
        f64,
    )> for PeerManager
{
    fn create_args(
        (
            network_channel,
            shell_channel,
            tokio_executor,
            identity,
            shell_compatibility_version,
            p2p_config,
            pow_target,
        ): (
            NetworkChannelRef,
            ShellChannelRef,
            Handle,
            Arc<Identity>,
            Arc<ShellCompatibilityVersion>,
            P2p,
            f64,
        ),
    ) -> Self {
        // resolve all bootstrap addresses
        // defaultlly init from bootstrap_peers
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

        PeerManager {
            network_channel,
            shell_channel,
            tokio_executor,
            bootstrap_addresses,
            config: p2p_config,
            proposer: None,
            local_node_info: Arc::new(LocalPeerInfo::new(
                listener_port,
                identity,
                shell_compatibility_version,
                pow_target,
            )),
            discovery_last: None,
        }
    }
}

impl Actor for PeerManager {
    type Msg = PeerManagerMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        subscribe_to_shell_shutdown(&self.shell_channel, ctx.myself());
        subscribe_to_network_commands(&self.network_channel, ctx.myself());
        subscribe_to_network_events(&self.network_channel, ctx.myself());

        let (proposer_tx, proposer_rx) = mpsc::channel();

        let mio_manager = MioManager::new(self.config.listener_port);
        self.proposer = Some(ProposerHandle::new(mio_manager.waker(), proposer_tx));

        let mut tezedge_state = TezedgeState::new(
            ctx.system.log().to_erased(),
            TezedgeConfig {
                port: self.config.listener_port,
                disable_mempool: self.config.disable_mempool,
                private_node: self.config.private_node,
                disable_quotas: false,
                disable_blacklist: self.config.disable_blacklist,
                min_connected_peers: self.config.peer_threshold.low,
                max_connected_peers: self.config.peer_threshold.high,
                max_pending_peers: self.config.peer_threshold.high,
                max_potential_peers: self.config.peer_threshold.high * 20,
                periodic_react_interval: Duration::from_millis(250),
                reset_quotas_interval: Duration::from_secs(5),
                peer_blacklist_duration: Duration::from_secs(8 * 60),
                peer_timeout: Duration::from_secs(8),
                pow_target: self.local_node_info.pow_target(),
            },
            (*self.local_node_info.identity()).clone(),
            (*self.local_node_info.version()).clone(),
            Default::default(),
            Instant::now(),
        );

        info!(ctx.system.log(), "Doing peer DNS lookup"; "bootstrap_addresses" => format!("{:?}", &self.bootstrap_addresses));
        tezedge_state.accept(ExtendPotentialPeersProposal {
            at: Instant::now(),
            peers: dbg!(dns_lookup_peers(
                &self.bootstrap_addresses,
                &ctx.system.log()
            ))
            .into_iter()
            .map(|x| x.into()),
        });

        let proposer = TezedgeProposer::new(
            TezedgeProposerConfig {
                wait_for_events_timeout: Some(Duration::from_millis(250)),
                events_limit: 1024,
            },
            tezedge_state,
            MioEvents::new(),
            mio_manager,
        );

        let network_channel = self.network_channel.clone();
        let log = ctx.system.log();

        // start to listen for incoming p2p connections
        std::thread::spawn(move || {
            run(proposer, proposer_rx, network_channel, &log);
        });
    }

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        self.receive(ctx, msg, sender);
    }
}

impl Receive<ShellChannelMsg> for PeerManager {
    type Msg = PeerManagerMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: ShellChannelMsg, _sender: Sender) {
        if let ShellChannelMsg::ShuttingDown(_) = msg {
            if let Some(proposer) = self.proposer.take() {
                drop(proposer.sender);
                proposer.waker.wake().unwrap();
            }
        }
    }
}

impl Receive<NetworkChannelMsg> for PeerManager {
    type Msg = PeerManagerMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: NetworkChannelMsg, _: Sender) {
        if let Some(proposer) = self.proposer.as_ref() {
            if let Err(err) = proposer.notify(msg) {
                warn!(ctx.system.log(), "Failed to notify proposer"; "reason" => format!("{:?}", err));
            }
        }
    }
}

fn run(
    mut proposer: TezedgeProposer<MioEvents, DefaultEffects, MioManager>,
    rx: mpsc::Receiver<ProposerMsg>,
    network_channel: NetworkChannelRef,
    log: &Logger,
) {
    let mut send_bootstrap_messages = vec![];
    loop {
        proposer.make_progress();

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

                    send_bootstrap_messages.push(peer_address);
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

        for peer in send_bootstrap_messages.drain(..) {
            proposer.send_message_to_peer_or_queue(Instant::now(), peer, PeerMessage::Bootstrap);
        }
        loop {
            match rx.try_recv() {
                Ok(ProposerMsg::NetworkChannel(msg)) => {
                    match msg {
                        NetworkChannelMsg::SendBootstrapPeers(peer_id) => {
                            trace!(log, "Received bootstrap message"; "peer_ip" => peer_id.address.to_string());
                            // TODO: send some potential peers
                            let msg = Arc::new(AdvertiseMessage::new(&vec![]).into());
                            network_channel.tell(
                                Publish {
                                    msg: NetworkChannelMsg::SendMessage(peer_id, msg),
                                    topic: NetworkChannelTopic::NetworkCommands.into(),
                                },
                                None,
                            );
                        }
                        NetworkChannelMsg::PeerStalled(peer_id) => {
                            proposer.disconnect_peer(Instant::now(), peer_id.address);
                        }
                        NetworkChannelMsg::BlacklistPeer(peer_id, reason) => {
                            proposer.blacklist_peer(Instant::now(), peer_id.address);
                        }
                        NetworkChannelMsg::SendMessage(peer_id, message) => {
                            proposer.send_message_to_peer_or_queue(
                                Instant::now(),
                                peer_id.address,
                                message.message.clone(),
                            );
                        }
                        _ => (),
                    }
                }
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
