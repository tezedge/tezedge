use std::fmt::{self, Debug};
use std::io::{self, Read, Write};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crypto::hash::CryptoboxPublicKeyHash;
use tla_sm::{Acceptor, GetRequests};

use crate::chunking::extendable_as_writable::ExtendableAsWritable;
use crate::proposals::{
    ExtendPotentialPeersProposal, NewPeerConnectProposal, PeerBlacklistProposal,
    PeerDisconnectProposal, PeerDisconnectedProposal, PeerReadableProposal, PeerWritableProposal,
    PendingRequestMsg, PendingRequestProposal, SendPeerMessageProposal, TickProposal,
};
use crate::{Effects, PeerAddress, TezedgeRequest, TezedgeStateWrapper};
use tezos_messages::p2p::encoding::peer::{PeerMessage, PeerMessageResponse};
use tezos_messages::p2p::encoding::prelude::{MetadataMessage, NetworkVersion};

pub mod mio_manager;

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
/// Right now it's simply responsible to manage p2p connections and
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

#[derive(Clone)]
pub struct TezedgeProposerConfig {
    pub wait_for_events_timeout: Option<Duration>,
    pub events_limit: usize,
}

/// TezedgeProposer wraps around [TezedgeState] and it is what connects
/// state machine to the outside world.
#[derive(Clone)]
pub struct TezedgeProposer<Es, Efs, M> {
    config: TezedgeProposerConfig,
    requests: Vec<TezedgeRequest>,
    notifications: Vec<Notification>,
    effects: Efs,
    pub state: TezedgeStateWrapper,
    pub events: Es,
    pub manager: M,
}

impl<S, NetE, Es, Efs, M> TezedgeProposer<Es, Efs, M>
where
    S: Read + Write,
    NetE: NetworkEvent + Debug,
    Efs: Effects + Debug,
    M: Manager<Stream = S, NetworkEvent = NetE, Events = Es>,
{
    pub fn new<T>(
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
        Self {
            config,
            requests: vec![],
            notifications: Vec::with_capacity(NOTIFICATIONS_OPTIMAL_CAPACITY),
            effects,
            state: state.into(),
            events,
            manager,
        }
        .init()
    }

    fn init(mut self) -> Self {
        // execute initial requests.
        Self::execute_requests(
            &mut self.effects,
            &mut self.requests,
            &mut self.notifications,
            &mut self.state,
            &mut self.manager,
        );
        self
    }

    pub fn assert_state(&self) {
        self.state.assert_state();
    }

    fn handle_event(
        event: Event<NetE>,
        effects: &mut Efs,
        requests: &mut Vec<TezedgeRequest>,
        notifications: &mut Vec<Notification>,
        state: &mut TezedgeStateWrapper,
        manager: &mut M,
    ) {
        match event {
            Event::Tick(at) => {
                state.accept(TickProposal { at, effects });
            }
            Event::Network(event) => {
                Self::handle_network_event(
                    &event,
                    effects,
                    requests,
                    notifications,
                    state,
                    manager,
                );
            }
        }
    }

    fn handle_event_ref<'a>(
        event: EventRef<'a, NetE>,
        effects: &mut Efs,
        requests: &mut Vec<TezedgeRequest>,
        notifications: &mut Vec<Notification>,
        state: &mut TezedgeStateWrapper,
        manager: &mut M,
    ) {
        match event {
            Event::Tick(at) => {
                state.accept(TickProposal { at, effects });
            }
            Event::Network(event) => {
                Self::handle_network_event(event, effects, requests, notifications, state, manager);
            }
        }
    }

    fn handle_network_event(
        event: &NetE,
        effects: &mut Efs,
        requests: &mut Vec<TezedgeRequest>,
        notifications: &mut Vec<Notification>,
        state: &mut TezedgeStateWrapper,
        manager: &mut M,
    ) {
        if event.is_server_event() {
            // we received event for the server (client opened tcp stream to us).
            loop {
                // as an optimization, execute requests only after 100
                // accepted new connections. We need to execute those
                // requests as they might include command to stop
                // listening for new connections or disconnect new peer,
                // if for example they are blacklisted.
                for _ in 0..100 {
                    match manager.accept_connection(&event) {
                        Some(peer) => {
                            state.accept(NewPeerConnectProposal {
                                effects,
                                at: event.time(),
                                peer: peer.address().clone(),
                            });
                            Self::handle_readiness_event(event, effects, state, peer);
                        }
                        None => return,
                    }
                }
                Self::execute_requests(effects, requests, notifications, state, manager);
            }
        } else {
            match manager.get_peer_for_event_mut(&event) {
                Some(peer) => Self::handle_readiness_event(event, effects, state, peer),
                None => {
                    // Should be impossible! If we receive an event for
                    // the peer, that peer must exist in manager.
                    // TODO: write error log.
                    return;
                }
            }
        };
    }

    fn handle_readiness_event(
        event: &NetE,
        effects: &mut Efs,
        state: &mut TezedgeStateWrapper,
        peer: &mut Peer<S>,
    ) {
        if event.is_read_closed() || event.is_write_closed() {
            state.accept(PeerDisconnectedProposal {
                effects,
                at: event.time(),
                peer: peer.address().clone(),
            });
            return;
        }

        if event.is_readable() {
            state.accept(PeerReadableProposal {
                effects,
                at: event.time(),
                peer: peer.address().clone(),
                stream: &mut peer.stream,
            });
        }

        if event.is_writable() {
            state.accept(PeerWritableProposal {
                effects,
                at: event.time(),
                peer: peer.address().clone(),
                stream: &mut peer.stream,
            });
        }
    }

    /// Grabs the requests from state machine, puts them in temporary
    /// container `requests`, then drains it and executes each request.
    fn execute_requests(
        effects: &mut Efs,
        requests: &mut Vec<TezedgeRequest>,
        notifications: &mut Vec<Notification>,
        state: &mut TezedgeStateWrapper,
        manager: &mut M,
    ) {
        state.get_requests(requests);

        for req in requests.drain(..) {
            match req {
                TezedgeRequest::StartListeningForNewPeers { req_id } => {
                    manager.start_listening_to_server_events();
                    state.accept(PendingRequestProposal {
                        effects,
                        req_id,
                        at: state.newest_time_seen(),
                        message: PendingRequestMsg::StartListeningForNewPeersSuccess,
                    });
                }
                TezedgeRequest::StopListeningForNewPeers { req_id } => {
                    manager.stop_listening_to_server_events();
                    state.accept(PendingRequestProposal {
                        effects,
                        req_id,
                        at: state.newest_time_seen(),
                        message: PendingRequestMsg::StopListeningForNewPeersSuccess,
                    });
                }
                TezedgeRequest::ConnectPeer { req_id, peer } => {
                    match manager.get_peer_or_connect_mut(&peer) {
                        Ok(_) => state.accept(PendingRequestProposal {
                            effects,
                            req_id,
                            at: state.newest_time_seen(),
                            message: PendingRequestMsg::ConnectPeerSuccess,
                        }),
                        Err(_) => state.accept(PendingRequestProposal {
                            effects,
                            req_id,
                            at: state.newest_time_seen(),
                            message: PendingRequestMsg::ConnectPeerError,
                        }),
                    }
                }
                TezedgeRequest::DisconnectPeer { req_id, peer } => {
                    manager.disconnect_peer(&peer);
                    state.accept(PendingRequestProposal {
                        effects,
                        req_id,
                        at: state.newest_time_seen(),
                        message: PendingRequestMsg::DisconnectPeerSuccess,
                    });
                    notifications.push(Notification::PeerDisconnected { peer });
                }
                TezedgeRequest::BlacklistPeer { req_id, peer } => {
                    manager.disconnect_peer(&peer);
                    state.accept(PendingRequestProposal {
                        effects,
                        req_id,
                        at: state.newest_time_seen(),
                        message: PendingRequestMsg::BlacklistPeerSuccess,
                    });
                    notifications.push(Notification::PeerBlacklisted { peer });
                }
                TezedgeRequest::PeerMessageReceived {
                    req_id,
                    peer,
                    message,
                } => {
                    state.accept(PendingRequestProposal {
                        effects,
                        req_id,
                        at: state.newest_time_seen(),
                        message: PendingRequestMsg::PeerMessageReceivedNotified,
                    });
                    notifications.push(Notification::MessageReceived { peer, message });
                }
                TezedgeRequest::NotifyHandshakeSuccessful {
                    req_id,
                    peer_address,
                    peer_public_key_hash,
                    metadata,
                    network_version,
                } => {
                    notifications.push(Notification::HandshakeSuccessful {
                        peer_address,
                        peer_public_key_hash,
                        metadata,
                        network_version,
                    });
                    state.accept(PendingRequestProposal {
                        effects,
                        req_id,
                        at: state.newest_time_seen(),
                        message: PendingRequestMsg::HandshakeSuccessfulNotified,
                    });
                }
            }
        }
    }

    fn wait_for_events(&mut self) {
        let wait_for_events_timeout = self.config.wait_for_events_timeout;
        self.manager
            .wait_for_events(&mut self.events, wait_for_events_timeout)
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
        if self.notifications.capacity() > NOTIFICATIONS_OPTIMAL_CAPACITY
            && self.notifications.is_empty()
        {
            // to preserve memory, shrink to optimal capacity.
            let _ = std::mem::replace(
                &mut self.notifications,
                Vec::with_capacity(NOTIFICATIONS_OPTIMAL_CAPACITY),
            );
        }
        self.wait_for_events();

        for event in self.events.into_iter() {
            Self::handle_event_ref(
                event,
                &mut self.effects,
                &mut self.requests,
                &mut self.notifications,
                &mut self.state,
                &mut self.manager,
            );
        }

        Self::execute_requests(
            &mut self.effects,
            &mut self.requests,
            &mut self.notifications,
            &mut self.state,
            &mut self.manager,
        );
    }

    pub fn make_progress_owned(&mut self)
    where
        for<'a> &'a Es: IntoIterator<Item = Event<NetE>>,
    {
        let time = Instant::now();
        self.wait_for_events();
        eprintln!("waited for events for: {}ms", time.elapsed().as_millis());

        let time = Instant::now();
        let mut count = 0;
        for event in self.events.into_iter() {
            count += 1;
            Self::handle_event(
                event,
                &mut self.effects,
                &mut self.requests,
                &mut self.notifications,
                &mut self.state,
                &mut self.manager,
            );
        }
        eprintln!(
            "handled {} events in: {}ms",
            count,
            time.elapsed().as_millis()
        );

        let time = Instant::now();
        Self::execute_requests(
            &mut self.effects,
            &mut self.requests,
            &mut self.notifications,
            &mut self.state,
            &mut self.manager,
        );
        eprintln!("executed requests in: {}ms", time.elapsed().as_millis());
    }

    pub fn extend_potential_peers<P>(&mut self, at: Instant, peers: P)
    where
        P: IntoIterator<Item = SocketAddr>,
    {
        self.state.accept(ExtendPotentialPeersProposal {
            effects: &mut self.effects,
            at,
            peers,
        })
    }

    pub fn disconnect_peer(&mut self, at: Instant, peer: PeerAddress) {
        let effects = &mut self.effects;
        self.state
            .accept(PeerDisconnectProposal { effects, at, peer })
    }

    pub fn blacklist_peer(&mut self, at: Instant, peer: PeerAddress) {
        let effects = &mut self.effects;
        self.state
            .accept(PeerBlacklistProposal { effects, at, peer })
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
        if let Some(peer) = self.manager.get_peer(&addr) {
            self.state.accept(SendPeerMessageProposal {
                effects: &mut self.effects,
                at,
                peer: addr,
                message,
            });
            // issue writable proposal in case stream is writable,
            // otherwise we would have to wait till we receive
            // writable event from mio.
            self.state.accept(PeerWritableProposal {
                effects: &mut self.effects,
                at,
                peer: addr,
                stream: &mut peer.stream,
            });
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
        let peer = match self.manager.get_peer(&addr) {
            Some(peer) => peer,
            None => return Err(io::Error::new(io::ErrorKind::NotFound, "peer not found!")),
        };

        self.state.accept(SendPeerMessageProposal {
            effects: &mut self.effects,
            at,
            peer: addr,
            message,
        });
        let mut send_buf = vec![];
        self.state.accept(PeerWritableProposal {
            effects: &mut self.effects,
            at,
            peer: addr,
            stream: &mut ExtendableAsWritable::from(&mut send_buf),
        });

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
