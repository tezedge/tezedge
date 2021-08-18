use std::fmt::{self, Debug};
use std::io::{self, Read, Write};
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use slog::Logger;

use crypto::hash::CryptoboxPublicKeyHash;
use tla_sm::{Acceptor, DefaultRecorder, GetRequests};

use crate::proposals::{
    ExtendPotentialPeersProposal, NewPeerConnectProposal, PeerBlacklistProposal,
    PeerDisconnectProposal, PeerDisconnectedProposal, PeerReadableProposal, PeerWritableProposal,
    PendingRequestMsg, PendingRequestProposal, RecordedProposal, SendPeerMessageProposal,
    TickProposal,
};
use crate::{Effects, PeerAddress, TezedgeRequest, TezedgeStateWrapper};
use tezos_messages::p2p::encoding::peer::{PeerMessage, PeerMessageResponse};
use tezos_messages::p2p::encoding::prelude::{MetadataMessage, NetworkVersion};

pub mod mio_manager;
pub mod proposal_loader;
pub mod proposal_persister;
use proposal_loader::ProposalLoader;
use proposal_persister::{ProposalPersister, ProposalPersisterHandle};

macro_rules! accept_proposal {
    ($state: expr, $proposal: expr, $proposal_persister: expr) => {
        let proposal = $proposal;

        match $proposal_persister.as_mut() {
            Some(persister) => {
                let mut recorder = proposal.default_recorder();
                $state.accept(recorder.record());
                persister.persist(recorder.finish_recording());
            }
            None => $state.accept(proposal),
        }
    };
}

const NOTIFICATIONS_OPTIMAL_CAPACITY: usize = 16;

/// Notification for state machine events.
#[derive(Debug, Clone)]
pub enum Notification {
    PeerDisconnected {
        peer: PeerAddress,
    },
    PeerBlacklisted {
        peer: PeerAddress,
    },
    MessageReceived {
        peer: PeerAddress,
        message: Arc<PeerMessageResponse>,
    },
    HandshakeSuccessful {
        peer_address: PeerAddress,
        peer_public_key_hash: CryptoboxPublicKeyHash,
        metadata: MetadataMessage,
        network_version: NetworkVersion,
    },
}

#[derive(Debug, Clone)]
pub enum Event<NetE> {
    Tick(Instant),
    Network(NetE),
}

impl<NetE> Event<NetE> {
    pub fn as_event_ref<'a>(&'a self) -> EventRef<'a, NetE> {
        match self {
            Self::Tick(e) => EventRef::Tick(*e),
            Self::Network(e) => EventRef::Network(e),
        }
    }
}

impl<NetE: NetworkEvent> Event<NetE> {
    pub fn time(&self) -> Instant {
        match self {
            Self::Tick(t) => t.clone(),
            Self::Network(e) => e.time(),
        }
    }
}

impl<NetE> From<NetE> for Event<NetE>
where
    NetE: NetworkEvent,
{
    fn from(event: NetE) -> Self {
        Self::Network(event)
    }
}

pub type EventRef<'a, NetE> = Event<&'a NetE>;

pub trait NetworkEvent {
    fn is_server_event(&self) -> bool;

    fn is_readable(&self) -> bool;
    fn is_writable(&self) -> bool;

    fn is_read_closed(&self) -> bool;
    fn is_write_closed(&self) -> bool;

    fn time(&self) -> Instant {
        Instant::now()
    }
}

pub trait Events {
    fn set_limit(&mut self, limit: usize);
}

pub struct Peer<S> {
    address: PeerAddress,
    pub stream: S,
}

impl<S> Peer<S> {
    pub fn new(address: PeerAddress, stream: S) -> Self {
        Self { address, stream }
    }

    pub fn address(&self) -> &PeerAddress {
        &self.address
    }
}

impl<S: Clone> Clone for Peer<S> {
    fn clone(&self) -> Self {
        Self {
            address: self.address.clone(),
            stream: self.stream.clone(),
        }
    }
}

impl<S: Debug> Debug for Peer<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Peer")
            .field("address", &self.address)
            .field("stream", &self.stream)
            .finish()
    }
}

/// Manager is an abstraction for [mio] layer.
///
/// Right now it's simply responsible to manage p2p connections.
pub trait Manager {
    type Stream: Read + Write;
    type NetworkEvent: NetworkEvent;
    type Events;

    fn start_listening_to_server_events(&mut self) -> io::Result<()>;
    fn stop_listening_to_server_events(&mut self);

    fn accept_connection(&mut self, event: &Self::NetworkEvent) -> Option<&mut Peer<Self::Stream>>;

    /// Blocks until events are available or until timeout passes.
    ///
    /// New events fill the passed `events_container`, removing all
    /// previous events from the container.
    ///
    /// If timeout passes, [Event::Tick] will be added to `events_container`.
    fn wait_for_events(&mut self, events_container: &mut Self::Events, timeout: Option<Duration>);

    fn get_peer(&mut self, address: &PeerAddress) -> Option<&mut Peer<Self::Stream>>;
    fn get_peer_or_connect_mut(
        &mut self,
        address: &PeerAddress,
    ) -> io::Result<&mut Peer<Self::Stream>>;
    fn get_peer_for_event_mut(
        &mut self,
        event: &Self::NetworkEvent,
    ) -> Option<&mut Peer<Self::Stream>>;

    fn disconnect_peer(&mut self, peer: &PeerAddress);
}

/// Internal clock of proposer.
#[derive(Clone)]
struct InternalClock {
    time: Instant,
    elapsed: Duration,
}

impl InternalClock {
    fn new(initial_time: Instant) -> Self {
        Self {
            time: initial_time,
            elapsed: Duration::new(0, 0),
        }
    }

    fn update(&mut self, new_time: Instant) -> &mut Self {
        if self.time >= new_time {
            // just to guard against panic.
            return self;
        }
        self.elapsed += new_time.duration_since(self.time);
        self.time = new_time;
        self
    }

    /// Result of every call of this method MUST be passed to state machine
    /// `TezedgeState` through proposal, otherwise internal clock will mess up.
    #[inline]
    fn take_elapsed(&mut self) -> Duration {
        std::mem::replace(&mut self.elapsed, Duration::new(0, 0))
    }
}

#[derive(Clone)]
pub struct TezedgeProposerConfig {
    pub wait_for_events_timeout: Option<Duration>,
    pub events_limit: usize,
    pub record: bool,
    pub replay: bool,
}

/// Tezedge proposer wihtout `events`.
#[derive(Clone)]
struct TezedgeProposerInner<Efs, M> {
    log: Option<Logger>,
    config: TezedgeProposerConfig,
    requests: Vec<TezedgeRequest>,
    notifications: Vec<Notification>,
    effects: Efs,
    time: InternalClock,
    proposal_persister: Option<ProposalPersisterHandle>,
    /// Proposal loader for replay.
    proposal_loader: Option<ProposalLoader>,
    state: TezedgeStateWrapper,
    manager: M,
}

impl<S, NetE, Es, Efs, M> TezedgeProposerInner<Efs, M>
where
    S: Read + Write,
    NetE: NetworkEvent + Debug,
    Efs: Effects + Debug,
    M: Manager<Stream = S, NetworkEvent = NetE, Events = Es>,
{
    #[inline]
    fn wait_for_events(&mut self, events: &mut Es) {
        self.manager
            .wait_for_events(events, self.config.wait_for_events_timeout)
    }

    // fn handle_event(&mut self, event: Event<NetE>) {
    //     match event {
    //         Event::Tick(at) => {
    //             accept_proposal!(
    //                 self.state,
    //                 TickProposal {
    //                     time_passed: self.update_time(at),
    //                     effects: &mut self.effects,
    //                 },
    //                 self.proposal_persister
    //             );
    //         }
    //         Event::Network(event) => {
    //             self.handle_network_event(&event);
    //         }
    //     }
    // }

    fn handle_event_ref<'a>(&mut self, event: EventRef<'a, NetE>) {
        match event {
            Event::Tick(at) => {
                accept_proposal!(
                    self.state,
                    TickProposal {
                        time_passed: self.time.update(at).take_elapsed(),
                        effects: &mut self.effects
                    },
                    self.proposal_persister
                );
            }
            Event::Network(event) => {
                self.handle_network_event(event);
            }
        }
    }

    fn handle_network_event(&mut self, event: &NetE) {
        self.time.update(event.time());
        if event.is_server_event() {
            // we received event for the server (client opened tcp stream to us).
            loop {
                // as an optimization, execute requests only after 100
                // accepted new connections. We need to execute those
                // requests as they might include command to stop
                // listening for new connections or disconnect new peer,
                // if for example they are blacklisted.
                for _ in 0..100 {
                    match self.manager.accept_connection(&event) {
                        Some(peer) => {
                            accept_proposal!(
                                self.state,
                                NewPeerConnectProposal {
                                    effects: &mut self.effects,
                                    time_passed: self.time.take_elapsed(),
                                    peer: peer.address().clone(),
                                },
                                self.proposal_persister
                            );
                            self.handle_readiness_event(event);
                        }
                        None => return,
                    }
                }
                self.execute_requests();
            }
        } else {
            self.handle_readiness_event(event);
        };
    }

    fn handle_readiness_event(&mut self, event: &NetE) {
        self.time.update(event.time());
        let peer = match self.manager.get_peer_for_event_mut(&event) {
            Some(peer) => peer,
            None => {
                // Should be impossible! If we receive an event for
                // the peer, that peer must exist in manager.
                // TODO: write error log.
                return;
            }
        };

        if event.is_read_closed() || event.is_write_closed() {
            accept_proposal!(
                self.state,
                PeerDisconnectedProposal {
                    effects: &mut self.effects,
                    time_passed: self.time.take_elapsed(),
                    peer: peer.address().clone(),
                },
                self.proposal_persister
            );
            return;
        }

        if event.is_readable() {
            accept_proposal!(
                self.state,
                PeerReadableProposal {
                    effects: &mut self.effects,
                    time_passed: self.time.take_elapsed(),
                    peer: peer.address().clone(),
                    stream: &mut peer.stream,
                },
                self.proposal_persister
            );
        }

        if event.is_writable() {
            accept_proposal!(
                self.state,
                PeerWritableProposal {
                    effects: &mut self.effects,
                    time_passed: self.time.take_elapsed(),
                    peer: peer.address().clone(),
                    stream: &mut peer.stream,
                },
                self.proposal_persister
            );
        }
    }

    /// Grabs the requests from state machine, puts them in temporary
    /// container `requests`, then drains it and executes each request.
    fn execute_requests(&mut self) {
        self.state.get_requests(&mut self.requests);

        for req in self.requests.drain(..) {
            match req {
                TezedgeRequest::StartListeningForNewPeers { req_id } => {
                    if self.config.replay {
                        continue;
                    }
                    let status = match self.manager.start_listening_to_server_events() {
                        Ok(_) => PendingRequestMsg::StartListeningForNewPeersSuccess,
                        Err(err) => {
                            PendingRequestMsg::StartListeningForNewPeersError { error: err.kind() }
                        }
                    };
                    accept_proposal!(
                        self.state,
                        PendingRequestProposal {
                            effects: &mut self.effects,
                            req_id,
                            time_passed: Default::default(),
                            message: status,
                        },
                        self.proposal_persister
                    );
                }
                TezedgeRequest::StopListeningForNewPeers { req_id } => {
                    if self.config.replay {
                        continue;
                    }
                    self.manager.stop_listening_to_server_events();
                    accept_proposal!(
                        self.state,
                        PendingRequestProposal {
                            effects: &mut self.effects,
                            req_id,
                            time_passed: Default::default(),
                            message: PendingRequestMsg::StopListeningForNewPeersSuccess,
                        },
                        self.proposal_persister
                    );
                }
                TezedgeRequest::ConnectPeer { req_id, peer } => {
                    if self.config.replay {
                        continue;
                    }
                    match self.manager.get_peer_or_connect_mut(&peer) {
                        Ok(_) => {
                            accept_proposal!(
                                self.state,
                                PendingRequestProposal {
                                    effects: &mut self.effects,
                                    req_id,
                                    time_passed: Default::default(),
                                    message: PendingRequestMsg::ConnectPeerSuccess,
                                },
                                self.proposal_persister
                            );
                        }
                        Err(_) => {
                            accept_proposal!(
                                self.state,
                                PendingRequestProposal {
                                    effects: &mut self.effects,
                                    req_id,
                                    time_passed: Default::default(),
                                    message: PendingRequestMsg::ConnectPeerError,
                                },
                                self.proposal_persister
                            );
                        }
                    }
                }
                TezedgeRequest::DisconnectPeer { req_id, peer } => {
                    self.notifications
                        .push(Notification::PeerDisconnected { peer });
                    if self.config.replay {
                        continue;
                    }
                    self.manager.disconnect_peer(&peer);
                    accept_proposal!(
                        self.state,
                        PendingRequestProposal {
                            effects: &mut self.effects,
                            req_id,
                            time_passed: Default::default(),
                            message: PendingRequestMsg::DisconnectPeerSuccess,
                        },
                        self.proposal_persister
                    );
                }
                TezedgeRequest::BlacklistPeer { req_id, peer } => {
                    self.notifications
                        .push(Notification::PeerBlacklisted { peer });
                    if self.config.replay {
                        continue;
                    }
                    self.manager.disconnect_peer(&peer);
                    accept_proposal!(
                        self.state,
                        PendingRequestProposal {
                            effects: &mut self.effects,
                            req_id,
                            time_passed: Default::default(),
                            message: PendingRequestMsg::BlacklistPeerSuccess,
                        },
                        self.proposal_persister
                    );
                }
                TezedgeRequest::PeerMessageReceived {
                    req_id,
                    peer,
                    message,
                } => {
                    self.notifications
                        .push(Notification::MessageReceived { peer, message });
                    if self.config.replay {
                        continue;
                    }
                    accept_proposal!(
                        self.state,
                        PendingRequestProposal {
                            effects: &mut self.effects,
                            req_id,
                            time_passed: Default::default(),
                            message: PendingRequestMsg::PeerMessageReceivedNotified,
                        },
                        self.proposal_persister
                    );
                }
                TezedgeRequest::NotifyHandshakeSuccessful {
                    req_id,
                    peer_address,
                    peer_public_key_hash,
                    metadata,
                    network_version,
                } => {
                    self.notifications.push(Notification::HandshakeSuccessful {
                        peer_address,
                        peer_public_key_hash,
                        metadata,
                        network_version,
                    });
                    if self.config.replay {
                        continue;
                    }
                    accept_proposal!(
                        self.state,
                        PendingRequestProposal {
                            effects: &mut self.effects,
                            req_id,
                            time_passed: Default::default(),
                            message: PendingRequestMsg::HandshakeSuccessfulNotified,
                        },
                        self.proposal_persister
                    );
                }
            }
        }
    }

    pub fn extend_potential_peers<P>(&mut self, at: Instant, peers: P)
    where
        P: IntoIterator<Item = SocketAddr>,
    {
        if self.config.replay {
            return;
        }

        accept_proposal!(
            self.state,
            ExtendPotentialPeersProposal {
                time_passed: self.time.update(at).take_elapsed(),
                effects: &mut self.effects,
                peers,
            },
            self.proposal_persister
        );
    }

    pub fn disconnect_peer(&mut self, at: Instant, peer: PeerAddress) {
        if self.config.replay {
            return;
        }

        accept_proposal!(
            self.state,
            PeerDisconnectProposal {
                time_passed: self.time.update(at).take_elapsed(),
                effects: &mut self.effects,
                peer
            },
            self.proposal_persister
        );
    }

    pub fn blacklist_peer(&mut self, at: Instant, peer: PeerAddress) {
        if self.config.replay {
            return;
        }

        accept_proposal!(
            self.state,
            PeerBlacklistProposal {
                time_passed: self.time.update(at).take_elapsed(),
                effects: &mut self.effects,
                peer
            },
            self.proposal_persister
        );
    }

    // TODO: Everything bellow this line is temporary until everything
    // is handled in TezedgeState.
    // ---------------------------------------------------------------

    /// Enqueue message to be sent to the peer.
    pub fn enqueue_send_message_to_peer(
        &mut self,
        at: Instant,
        addr: PeerAddress,
        message: PeerMessage,
    ) {
        if self.config.replay {
            return;
        }
        self.time.update(at);

        if let Some(peer) = self.manager.get_peer(&addr) {
            accept_proposal!(
                self.state,
                SendPeerMessageProposal {
                    effects: &mut self.effects,
                    time_passed: self.time.take_elapsed(),
                    peer: addr,
                    message,
                },
                self.proposal_persister
            );
            // issue proposal just in case stream is writable,
            // otherwise we would have to wait till we receive
            // writable event from mio.
            accept_proposal!(
                self.state,
                PeerWritableProposal {
                    effects: &mut self.effects,
                    time_passed: Default::default(),
                    peer: addr,
                    stream: &mut peer.stream,
                },
                self.proposal_persister
            );
        } else {
            eprintln!("queueing send message failed since peer not found!");
        }
    }

    /// Send and flush message and block until entire message is sent.
    #[cfg(feature = "blocking")]
    pub fn blocking_send(
        &mut self,
        at: Instant,
        addr: PeerAddress,
        message: PeerMessage,
    ) -> io::Result<()> {
        use crate::chunking::extendable_as_writable::ExtendableAsWritable;

        let peer = match self.manager.get_peer(&addr) {
            Some(peer) => peer,
            None => return Err(io::Error::new(io::ErrorKind::NotFound, "peer not found!")),
        };

        accept_proposal!(
            self.state,
            SendPeerMessageProposal {
                effects: &mut self.effects,
                time_passed: self.time.update(at).take_elapsed(),
                peer: addr,
                message,
            },
            self.proposal_persister
        );
        let mut send_buf = vec![];
        accept_proposal!(
            self.state,
            PeerWritableProposal {
                effects: &mut self.effects,
                time_passed: Default::default(),
                peer: addr,
                stream: &mut ExtendableAsWritable::from(&mut send_buf),
            },
            self.proposal_persister
        );

        let mut buf = &send_buf[..];

        while buf.len() > 0 {
            match peer.stream.write(buf) {
                Ok(len) => buf = &buf[len..],
                Err(err) => {
                    match err.kind() {
                        io::ErrorKind::WouldBlock => std::thread::yield_now(),
                        _ => return Err(err),
                    };
                }
            }
        }

        Ok(())
    }

    /// Drain notifications.
    ///
    /// Must be called! Without draining this, notifications queue will
    /// grow infinitely large.
    pub fn take_notifications<'a>(&'a mut self) -> std::vec::Drain<'a, Notification> {
        self.notifications.drain(..)
    }
}

/// TezedgeProposer wraps around [TezedgeState] and it is what connects
/// state machine to the outside world.
#[derive(Clone)]
pub struct TezedgeProposer<Es, Efs, M> {
    inner: TezedgeProposerInner<Efs, M>,
    events: Es,
}

impl<S, NetE, Es, Efs, M> TezedgeProposer<Es, Efs, M>
where
    S: Read + Write,
    NetE: NetworkEvent + Debug,
    Efs: Effects + Debug,
    M: Manager<Stream = S, NetworkEvent = NetE, Events = Es>,
{
    pub fn new<T>(
        log: Option<Logger>,
        initial_time: Instant,
        config: TezedgeProposerConfig,
        effects: Efs,
        state: T,
        mut events: Es,
        manager: M,
    ) -> Self
    where
        T: Into<TezedgeStateWrapper>,
        Es: Events,
    {
        events.set_limit(config.events_limit);
        let proposal_persister = if config.record && !config.replay {
            Some(ProposalPersister::start())
        } else {
            None
        };
        let proposal_loader = if config.replay {
            ProposalLoader::new()
        } else {
            None
        };

        Self {
            inner: TezedgeProposerInner {
                log,
                config,
                requests: vec![],
                notifications: Vec::with_capacity(NOTIFICATIONS_OPTIMAL_CAPACITY),
                effects,
                state: state.into(),
                manager,
                time: InternalClock::new(initial_time),
                proposal_persister,
                proposal_loader,
            },
            events,
        }
        .init()
    }

    pub fn manager(&self) -> &M {
        &self.inner.manager
    }

    pub fn manager_mut(&mut self) -> &mut M {
        &mut self.inner.manager
    }

    pub fn state(&self) -> &TezedgeStateWrapper {
        &self.inner.state
    }

    fn init(mut self) -> Self {
        // execute initial requests.
        self.inner.execute_requests();
        self
    }

    pub fn assert_state(&self) {
        self.inner.state.assert_state();
    }

    #[inline]
    fn wait_for_events(&mut self) {
        self.inner.wait_for_events(&mut self.events)
    }

    fn replay_database<P>(&mut self, path: P)
    where
        P: AsRef<Path> + fmt::Debug,
    {
        use std::collections::{HashMap, HashSet};
        use std::io::Cursor;
        use tezedge_recorder::database::{
            DatabaseNew,
            rocks::Db,
            rocks_utils::SyscallKind,
        };
        use crate::proposals as pr;
        use self::TezedgeRequest::*;

        #[derive(Default)]
        struct MockStream {
            incoming: Cursor<Vec<u8>>,
            outgoing: Cursor<Vec<u8>>,
        }

        impl MockStream {
            pub fn put(&mut self, slice: &[u8]) {
                self.incoming.get_mut().extend_from_slice(slice);
            }

            pub fn take(&mut self, buf: &mut [u8]) -> usize {
                self.outgoing.read(buf).unwrap()
            }
        }

        impl Read for MockStream {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                self.incoming.read(buf)
            }
        }

        impl Write for MockStream {
            fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                self.outgoing.get_mut().extend_from_slice(buf);
                Ok(buf.len())
            }

            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        let requests = |state: &TezedgeStateWrapper| {
            let mut req = vec![];
            state.get_requests(&mut req);
            req
        };
        let logger = self.inner.log.as_ref().unwrap();

        let db = Db::open(&path, false, None, None)
            .unwrap_or_else(|error| panic!("database {:?} is invalid: {:?}", path, error));
        let items = db.syscall_metadata_iterator(0)
            .expect("failed to read syscalls metadata from the db");
        let mut last_time = None;
        let mut was_closed = HashSet::new();
        let mut peer_streams = HashMap::<SocketAddr, MockStream>::new();
        let mut cnt = 2;
        for item in items {
            let item = item.expect("failed to retrieve an item from the db");
            if last_time.is_none() {
                last_time = Some(item.timestamp);

                let mut req = requests(&self.inner.state).into_iter();
                loop {
                    match req.next() {
                        Some(StartListeningForNewPeers { req_id }) => {
                            self.inner.state.accept(pr::PendingRequestProposal {
                                effects: &mut self.inner.effects,
                                req_id,
                                time_passed: item.timestamp - last_time.unwrap(),
                                message: PendingRequestMsg::StartListeningForNewPeersSuccess,
                            });
                            break;
                        },
                        Some(_) => (),
                        None => break,
                    }
                }

                let bootstrap_peers = vec![
                    "[::ffff:45.55.54.70]:9733",
                    "[::ffff:34.95.63.80]:9732",
                    "[::ffff:212.47.230.138]:9732",
                    "[2001:bc8:47b0:907::1]:9732",
                ];
                self.inner.state.accept(pr::ExtendPotentialPeersProposal {
                    effects: &mut self.inner.effects,
                    time_passed: Duration::from_millis(1),
                    peers: bootstrap_peers.into_iter().map(|s| s.parse().unwrap()),
                });
            }

            //slog::info!(logger, "replaying: {:?}", item);
            let time_passed = item.timestamp - last_time.unwrap();
            last_time = Some(item.timestamp);
            let addr = item.socket_address.unwrap();

            let req = requests(&self.inner.state);
            //slog::info!(logger, "requests: {:?}", req);
            for req in &req {
                if let &BlacklistPeer { req_id, peer } = req {
                    if was_closed.contains(&peer.into()) {
                        self.inner.state.accept(pr::PendingRequestProposal {
                            effects: &mut self.inner.effects,
                            req_id,
                            time_passed: Duration::default(),
                            message: PendingRequestMsg::BlacklistPeerSuccess,
                        });
                        slog::warn!(logger, "proposal {} BlacklistPeerSuccess {}", cnt, peer.ipv6_mapped());
                        cnt += 1;
                    }
                }
                if let &NotifyHandshakeSuccessful { req_id, .. } = req {
                    self.inner.state.accept(pr::PendingRequestProposal {
                        effects: &mut self.inner.effects,
                        req_id,
                        time_passed: Duration::default(),
                        message: PendingRequestMsg::HandshakeSuccessfulNotified,
                    });
                    slog::warn!(logger, "proposal {} HandshakeSuccessfulNotified", cnt);
                    cnt += 1;
                }
                if let &PeerMessageReceived { req_id, .. } = req {
                    self.inner.state.accept(pr::PendingRequestProposal {
                        effects: &mut self.inner.effects,
                        req_id,
                        time_passed: Duration::default(),
                        message: PendingRequestMsg::PeerMessageReceivedNotified,
                    });
                    slog::warn!(logger, "proposal {} PeerMessageReceivedNotified", cnt);
                    cnt += 1;
                }
            }
            let mut req = req.into_iter();
            let mut consumed = false;
            let response = loop {
                match req.next() {
                    None => {
                        // did not satisfy any request
                        break None;
                    },
                    Some(req) => match (req, &item.inner) {
                        (
                            DisconnectPeer { req_id, peer },
                            SyscallKind::Close,
                        ) if addr == peer.ipv6_mapped() => {
                            consumed = true;
                            was_closed.insert(addr);
                            break Some((req_id, PendingRequestMsg::DisconnectPeerSuccess));
                        },
                        (
                            ConnectPeer { req_id, peer },
                            SyscallKind::Connect(result),
                        ) if addr == peer.ipv6_mapped() => {
                            consumed = true;
                            let message = match result {
                                Ok(()) => PendingRequestMsg::ConnectPeerSuccess,
                                Err(-115) => PendingRequestMsg::ConnectPeerSuccess,
                                Err(_) => {
                                    was_closed.insert(addr);
                                    PendingRequestMsg::ConnectPeerError
                                },
                            };
                            break Some((req_id, message));
                        },
                        _ => {
                            // cannot satisfy this request, but maybe can satisfy
                            // another further request, proceed
                        },
                    }
                }
            };
            if let Some((req_id, message)) = response {
                self.inner.state.accept(pr::PendingRequestProposal {
                    effects: &mut self.inner.effects,
                    req_id,
                    time_passed,
                    message: message.clone(),
                });
                slog::warn!(logger, "proposal {} {:?} {}", cnt, message, addr);
                cnt += 1;
            }

            if !consumed {
                match &item.inner {
                    SyscallKind::Write(result) => {
                        if result.is_err() {
                            // hard to say what should we do here,
                            // state machine should try to write, but it should receive an error
                        }
                        match result {
                            Ok(recorded_data) => {
                                let mut actual_data = recorded_data.clone();

                                let stream = peer_streams.entry(addr).or_default();
                                if stream.take(&mut actual_data) == 0 {
                                    self.inner.state.accept(pr::PeerWritableProposal {
                                        effects: &mut self.inner.effects,
                                        time_passed,
                                        peer: addr.into(),
                                        stream,
                                    });
                                    stream.take(&mut actual_data);
                                }
                                slog::warn!(logger, "proposal {} write {}, {}", cnt, addr, hex::encode(&actual_data));
                                cnt += 1;
                                // we expect the data is equal to the data state machine is written
                                let equal = recorded_data == &actual_data;

                                assert!(equal);
                            },
                            Err(code) => {
                                let _ = code;
                                // TODO:
                            },
                        }
                    },
                    SyscallKind::Read(result) => {
                        let stream = peer_streams.entry(addr).or_default();
                        match result {
                            Ok(data) => {
                                stream.put(data);
                                self.inner.state.accept(pr::PeerReadableProposal {
                                    effects: &mut self.inner.effects,
                                    time_passed,
                                    peer: addr.into(),
                                    stream,
                                });
                                slog::warn!(logger, "proposal {} read {}, {}", cnt, addr, hex::encode(data));
                                cnt += 1;
                            },
                            Err(-11) => {
                                // TODO:
                            },
                            Err(code) => {
                                // hard to say what should we do here, mock stream
                                let _ = code;
                            },
                        };
                    },
                    SyscallKind::Close => {
                        self.inner.state.accept(pr::PeerDisconnectedProposal {
                            effects: &mut self.inner.effects,
                            time_passed,
                            peer: addr.into(),
                        });
                        slog::warn!(logger, "proposal {} close {}", cnt, addr);
                        cnt += 1;
                    },
                    _ => {
                        // should not happens
                        slog::error!(logger, "unused: {:?}", item);
                    },
                }
            }
        }
    }

    fn replay_proposals(&mut self) {
        if let Some(loader) = self.inner.proposal_loader.as_mut() {
            let mut proposal = match loader.next() {
                Some(result) => result.unwrap(),
                None => {
                    self.inner.proposal_loader = None;
                    return;
                }
            };
            use RecordedProposal::*;

            let time_passed;
            match &mut proposal {
                ExtendPotentialPeersProposal(proposal) => {
                    time_passed = proposal.time_passed;
                    self.inner.state.accept(proposal);
                }
                NewPeerConnectProposal(proposal) => {
                    time_passed = proposal.time_passed;
                    self.inner.state.accept(proposal);
                }
                PeerBlacklistProposal(proposal) => {
                    time_passed = proposal.time_passed;
                    self.inner.state.accept(proposal);
                }
                PeerDisconnectProposal(proposal) => {
                    time_passed = proposal.time_passed;
                    self.inner.state.accept(proposal);
                }
                PeerDisconnectedProposal(proposal) => {
                    time_passed = proposal.time_passed;
                    self.inner.state.accept(proposal);
                }
                PeerMessageProposal(proposal) => {
                    time_passed = proposal.time_passed;
                    self.inner.state.accept(proposal);
                }
                PeerReadableProposal(proposal) => {
                    time_passed = proposal.time_passed;
                    self.inner.state.accept(proposal);
                }
                PeerWritableProposal(proposal) => {
                    time_passed = proposal.time_passed;
                    self.inner.state.accept(proposal);
                }
                PendingRequestProposal(proposal) => {
                    time_passed = proposal.time_passed;
                    self.inner.state.accept(proposal);
                }
                SendPeerMessageProposal(proposal) => {
                    time_passed = proposal.time_passed;
                    self.inner.state.accept(proposal);
                }
                TickProposal(proposal) => {
                    time_passed = proposal.time_passed;
                    self.inner.state.accept(proposal);
                }
            };
            std::thread::sleep(time_passed);
        } else {
            let mut file = std::fs::File::create("state_after_replay").unwrap();
            write!(file, "{:?}", self.state()).unwrap();
            file.flush().unwrap();
            file.sync_all().unwrap();
            panic!("proposal replay finished!");
        }
    }

    /// Main driving function for [TezedgeProposer].
    ///
    /// It asks [Manager] to wait for events, handles them as they become
    /// available and executes requests incoming from the [TezedgeState].
    ///
    /// Needs to be called continously in an infinite loop.
    pub fn make_progress(&mut self)
    where
        for<'a> &'a Es: IntoIterator<Item = EventRef<'a, NetE>>,
    {
        if self.inner.notifications.capacity() > NOTIFICATIONS_OPTIMAL_CAPACITY
            && self.inner.notifications.is_empty()
        {
            // to preserve memory, shrink to optimal capacity.
            let _ = std::mem::replace(
                &mut self.inner.notifications,
                Vec::with_capacity(NOTIFICATIONS_OPTIMAL_CAPACITY),
            );
        }
        if self.inner.config.replay {
            if let Ok(path) = std::env::var("REPLAY_DB") {
                self.replay_database(path);
                let mut file = std::fs::File::create("target/state_after_replay").unwrap();
                write!(file, "{:?}", self.state()).unwrap();
                file.flush().unwrap();
                file.sync_all().unwrap();
                std::process::exit(0);
            } else {
                self.replay_proposals();
            }
        } else {
            self.wait_for_events();

            for event in self.events.into_iter() {
                self.inner.handle_event_ref(event);
            }
        }

        self.inner.execute_requests();
    }

    // pub fn make_progress_owned(&mut self)
    // where
    //     for<'a> &'a Es: IntoIterator<Item = Event<NetE>>,
    // {
    //     self.wait_for_events();

    //     for event in self.events.into_iter() {
    //         self.inner.handle_event(event);
    //     }

    //     self.inner.execute_requests();
    // }

    #[inline]
    pub fn extend_potential_peers<P>(&mut self, at: Instant, peers: P)
    where
        P: IntoIterator<Item = SocketAddr>,
    {
        self.inner.extend_potential_peers(at, peers)
    }

    #[inline]
    pub fn disconnect_peer(&mut self, at: Instant, peer: PeerAddress) {
        self.inner.disconnect_peer(at, peer)
    }

    #[inline]
    pub fn blacklist_peer(&mut self, at: Instant, peer: PeerAddress) {
        self.inner.blacklist_peer(at, peer)
    }

    // TODO: Everything bellow this line is temporary until everything
    // is handled in TezedgeState.
    // ---------------------------------------------------------------

    /// Enqueue message to be sent to the peer.
    #[inline]
    pub fn enqueue_send_message_to_peer(
        &mut self,
        at: Instant,
        addr: PeerAddress,
        message: PeerMessage,
    ) {
        self.inner.enqueue_send_message_to_peer(at, addr, message)
    }

    /// Send and flush message and block until entire message is sent.
    #[cfg(feature = "blocking")]
    pub fn blocking_send(
        &mut self,
        at: Instant,
        addr: PeerAddress,
        message: PeerMessage,
    ) -> io::Result<()> {
        self.inner.blocking_send(at, addr, message)
    }

    /// Drain notifications.
    ///
    /// Must be called! Without draining this, notifications queue will
    /// grow infinitely large.
    #[inline]
    pub fn take_notifications<'a>(&'a mut self) -> std::vec::Drain<'a, Notification> {
        self.inner.take_notifications()
    }
}
