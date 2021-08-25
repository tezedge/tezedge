// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use slog::Logger;
use std::fmt::{self, Debug};
use std::io::{self, Read, Write};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crypto::hash::CryptoboxPublicKeyHash;
use tla_sm::{Acceptor, DefaultRecorder, GetRequests};

use tezedge_state::proposals::{
    ExtendPotentialPeersProposal, NewPeerConnectProposal, PeerBlacklistProposal,
    PeerDisconnectProposal, PeerDisconnectedProposal, PeerReadableProposal, PeerWritableProposal,
    PendingRequestMsg, PendingRequestProposal, RecordedProposal, SendPeerMessageProposal,
    TickProposal,
};
use tezedge_state::{Effects, PeerAddress, TezedgeRequest, TezedgeStateWrapper};
use tezos_messages::p2p::encoding::peer::{PeerMessage, PeerMessageResponse};
use tezos_messages::p2p::encoding::prelude::{MetadataMessage, NetworkVersion};

mod event;
pub use event::*;

mod internal_clock;
pub use internal_clock::*;

pub mod mio_manager;
pub mod proposal_loader;
use proposal_loader::{NoopProposalLoader, ProposalLoader};

pub mod proposal_persister;
use proposal_persister::{NoopProposalPersister, ProposalPersister};

macro_rules! accept_proposal {
    ($state: expr, $proposal: expr, $proposal_persister: expr) => {
        let proposal = $proposal;

        match $proposal_persister.as_mut() {
            Some(persister) => {
                let mut recorder = proposal.default_recorder();
                $state.accept(recorder.record());
                persister.persist_proposal(recorder.finish_recording());
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

#[derive(Clone)]
pub struct TezedgeProposerConfig {
    pub wait_for_events_timeout: Option<Duration>,
    pub events_limit: usize,
}

/// Tezedge proposer wihtout `events`.
#[derive(Clone)]
struct TezedgeProposerInner<Efs, M, PP = NoopProposalPersister, PL = NoopProposalLoader> {
    log: Logger,
    config: TezedgeProposerConfig,
    requests: Vec<TezedgeRequest>,
    notifications: Vec<Notification>,
    effects: Efs,
    time: InternalClock,
    proposal_persister: Option<PP>,
    /// Proposal loader for replay.
    proposal_loader: Option<PL>,
    state: TezedgeStateWrapper,
    manager: M,
}

impl<S, NetE, Es, Efs, M, PP, PL> TezedgeProposerInner<Efs, M, PP, PL>
where
    S: Read + Write,
    NetE: NetworkEvent + Debug,
    Efs: Effects + Debug,
    M: Manager<Stream = S, NetworkEvent = NetE, Events = Es>,
    PP: ProposalPersister,
    PL: ProposalLoader,
{
    #[inline]
    fn wait_for_events(&mut self, events: &mut Es) {
        self.manager
            .wait_for_events(events, self.config.wait_for_events_timeout)
    }

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
                accept_proposal!(
                    self.state,
                    TickProposal {
                        time_passed: self.time.update(event.time()).take_elapsed(),
                        effects: &mut self.effects
                    },
                    self.proposal_persister
                );
                self.handle_network_event(event);
            }
        }
    }

    fn handle_network_event(&mut self, event: &NetE) {
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
        } else if !event.is_waker_event() {
            self.handle_readiness_event(event);
        }
    }

    fn handle_readiness_event(&mut self, event: &NetE) {
        let peer = match self.manager.get_peer_for_event_mut(&event) {
            Some(peer) => peer,
            None => {
                // Should be impossible! If we receive an event for
                // the peer, that peer must exist in manager. If this
                // happens, mio is misbehaving.
                // Not really an issue even if this happens though.
                slog::warn!(&self.log, "Peer not found for NetworkEvent");
                return;
            }
        };

        if event.is_read_closed() || event.is_write_closed() {
            accept_proposal!(
                self.state,
                PeerDisconnectedProposal {
                    effects: &mut self.effects,
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
                    if self.proposal_loader.is_some() {
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
                            message: status,
                        },
                        self.proposal_persister
                    );
                }
                TezedgeRequest::StopListeningForNewPeers { req_id } => {
                    if self.proposal_loader.is_some() {
                        continue;
                    }
                    self.manager.stop_listening_to_server_events();
                    accept_proposal!(
                        self.state,
                        PendingRequestProposal {
                            effects: &mut self.effects,
                            req_id,
                            message: PendingRequestMsg::StopListeningForNewPeersSuccess,
                        },
                        self.proposal_persister
                    );
                }
                TezedgeRequest::ConnectPeer { req_id, peer } => {
                    if self.proposal_loader.is_some() {
                        continue;
                    }
                    match self.manager.get_peer_or_connect_mut(&peer) {
                        Ok(_) => {
                            accept_proposal!(
                                self.state,
                                PendingRequestProposal {
                                    effects: &mut self.effects,
                                    req_id,
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
                    if self.proposal_loader.is_some() {
                        continue;
                    }
                    self.manager.disconnect_peer(&peer);
                    accept_proposal!(
                        self.state,
                        PendingRequestProposal {
                            effects: &mut self.effects,
                            req_id,
                            message: PendingRequestMsg::DisconnectPeerSuccess,
                        },
                        self.proposal_persister
                    );
                }
                TezedgeRequest::BlacklistPeer { req_id, peer } => {
                    self.notifications
                        .push(Notification::PeerBlacklisted { peer });
                    if self.proposal_loader.is_some() {
                        continue;
                    }
                    self.manager.disconnect_peer(&peer);
                    accept_proposal!(
                        self.state,
                        PendingRequestProposal {
                            effects: &mut self.effects,
                            req_id,
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
                    if self.proposal_loader.is_some() {
                        continue;
                    }
                    accept_proposal!(
                        self.state,
                        PendingRequestProposal {
                            effects: &mut self.effects,
                            req_id,
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
                    if self.proposal_loader.is_some() {
                        continue;
                    }
                    accept_proposal!(
                        self.state,
                        PendingRequestProposal {
                            effects: &mut self.effects,
                            req_id,
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
        if self.proposal_loader.is_some() {
            return;
        }

        accept_proposal!(
            self.state,
            TickProposal {
                effects: &mut self.effects,
                time_passed: self.time.update(at).take_elapsed(),
            },
            self.proposal_persister
        );

        accept_proposal!(
            self.state,
            ExtendPotentialPeersProposal {
                effects: &mut self.effects,
                peers,
            },
            self.proposal_persister
        );
    }

    pub fn disconnect_peer(&mut self, at: Instant, peer: PeerAddress) {
        if self.proposal_loader.is_some() {
            return;
        }

        accept_proposal!(
            self.state,
            TickProposal {
                effects: &mut self.effects,
                time_passed: self.time.update(at).take_elapsed(),
            },
            self.proposal_persister
        );

        accept_proposal!(
            self.state,
            PeerDisconnectProposal {
                effects: &mut self.effects,
                peer
            },
            self.proposal_persister
        );
    }

    pub fn blacklist_peer(&mut self, at: Instant, peer: PeerAddress) {
        if self.proposal_loader.is_some() {
            return;
        }

        accept_proposal!(
            self.state,
            TickProposal {
                effects: &mut self.effects,
                time_passed: self.time.update(at).take_elapsed(),
            },
            self.proposal_persister
        );

        accept_proposal!(
            self.state,
            PeerBlacklistProposal {
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
        if self.proposal_loader.is_some() {
            return;
        }
        accept_proposal!(
            self.state,
            TickProposal {
                effects: &mut self.effects,
                time_passed: self.time.update(at).take_elapsed(),
            },
            self.proposal_persister
        );

        if let Some(peer) = self.manager.get_peer(&addr) {
            accept_proposal!(
                self.state,
                SendPeerMessageProposal {
                    effects: &mut self.effects,
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
                    peer: addr,
                    stream: &mut peer.stream,
                },
                self.proposal_persister
            );
        } else {
            slog::warn!(&self.log, "Enqueueing message to be sent failed"; "peer" => addr.to_string(), "message_type" => message.get_type_str());
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
        use tezedge_state::chunking::extendable_as_writable::ExtendableAsWritable;

        accept_proposal!(
            self.state,
            TickProposal {
                effects: &mut self.effects,
                time_passed: self.time.update(at).take_elapsed(),
            },
            self.proposal_persister
        );

        let peer = match self.manager.get_peer(&addr) {
            Some(peer) => peer,
            None => return Err(io::Error::new(io::ErrorKind::NotFound, "peer not found!")),
        };

        accept_proposal!(
            self.state,
            SendPeerMessageProposal {
                effects: &mut self.effects,
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
pub struct TezedgeProposer<Es, Efs, M, PP = NoopProposalPersister, PL = NoopProposalLoader> {
    inner: TezedgeProposerInner<Efs, M, PP, PL>,
    events: Es,
}

impl<S, NetE, Es, Efs, M, PP, PL> TezedgeProposer<Es, Efs, M, PP, PL>
where
    S: Read + Write,
    NetE: NetworkEvent + Debug,
    Efs: Effects + Debug,
    M: Manager<Stream = S, NetworkEvent = NetE, Events = Es>,
    PP: ProposalPersister,
    PL: ProposalLoader,
{
    pub fn new<T>(
        initial_time: Instant,
        log: Logger,
        config: TezedgeProposerConfig,
        effects: Efs,
        state: T,
        mut events: Es,
        manager: M,
        proposal_persister: Option<PP>,
        proposal_loader: Option<PL>,
    ) -> Self
    where
        T: Into<TezedgeStateWrapper>,
        Es: Events,
    {
        events.set_limit(config.events_limit);

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

            match &mut proposal {
                ExtendPotentialPeersProposal(proposal) => {
                    self.inner.state.accept(proposal);
                }
                NewPeerConnectProposal(proposal) => {
                    self.inner.state.accept(proposal);
                }
                PeerBlacklistProposal(proposal) => {
                    self.inner.state.accept(proposal);
                }
                PeerDisconnectProposal(proposal) => {
                    self.inner.state.accept(proposal);
                }
                PeerDisconnectedProposal(proposal) => {
                    self.inner.state.accept(proposal);
                }
                PeerMessageProposal(proposal) => {
                    self.inner.state.accept(proposal);
                }
                PeerReadableProposal(proposal) => {
                    self.inner.state.accept(proposal);
                }
                PeerWritableProposal(proposal) => {
                    self.inner.state.accept(proposal);
                }
                PendingRequestProposal(proposal) => {
                    self.inner.state.accept(proposal);
                }
                SendPeerMessageProposal(proposal) => {
                    self.inner.state.accept(proposal);
                }
                TickProposal(proposal) => {
                    // TODO: We have to do this sleep because of non-deterministic
                    // parts that are outside of state machine, like riker.
                    // Once those parts are moved in, this should be removed.
                    std::thread::sleep(proposal.time_passed);
                    self.inner.state.accept(proposal);
                }
            };
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
        if self.inner.proposal_loader.is_some() {
            self.replay_proposals();
        } else {
            self.wait_for_events();

            for event in self.events.into_iter() {
                self.inner.handle_event_ref(event);
            }
        }

        self.inner.execute_requests();
    }

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
