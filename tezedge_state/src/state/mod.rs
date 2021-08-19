// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use slog::Logger;
use std::collections::BTreeSet;
use std::fmt::{self, Debug};
use std::time::{Duration, SystemTime};

use crypto::hash::ChainId;
use tezos_identity::Identity;
use tezos_messages::p2p::encoding::prelude::{
    GetCurrentBranchMessage, MetadataMessage, PeerMessage,
};
pub use tla_sm::{Acceptor, GetRequests, Proposal};

use crate::peer_address::PeerListenerAddress;
use crate::proposals::InvalidProposalError;
use crate::{Effects, PeerAddress, Port, ShellCompatibilityVersion};

mod assert_state;

mod requests;
pub use requests::*;

mod quotas;
pub(crate) use quotas::*;

mod pending_peers;
pub(crate) use pending_peers::*;

mod connected_peers;
pub(crate) use connected_peers::*;

mod blacklisted_peers;
pub(crate) use blacklisted_peers::*;

#[derive(Debug)]
pub struct NotMatchingAddress;

#[derive(Debug, Clone)]
pub struct TezedgeConfig {
    pub port: Port,
    pub disable_mempool: bool,
    pub private_node: bool,
    pub min_connected_peers: usize,
    pub max_connected_peers: usize,
    pub max_pending_peers: usize,
    pub max_potential_peers: usize,
    pub periodic_react_interval: Duration,
    pub reset_quotas_interval: Duration,
    /// Not used at the moment!
    // TODO: use disable_quotas in ThrottleQuota.
    pub disable_quotas: bool,
    pub disable_blacklist: bool,
    pub peer_blacklist_duration: Duration,
    pub peer_timeout: Duration,
    pub pow_target: f64,
}

#[derive(Debug, Clone, Copy)]
pub enum P2pState {
    /// Minimum number of connected peers **not** reached.
    /// Maximum number of pending connections **not** reached.
    Pending,

    /// Minimum number of connected peers **not** reached.
    /// Maximum number of pending connections reached.
    PendingFull,

    /// Minimum number of connected peers reached.
    /// Maximum number of connected peers **not** reached.
    /// Maximum number of pending connections **not** reached.
    Ready,

    /// Minimum number of connected peers reached.
    /// Maximum number of connected peers **not** reached.
    /// Maximum number of pending peers reached.
    ReadyFull,

    /// Maximum number of connected peers reached.
    ReadyMaxed,
}

impl P2pState {
    pub fn is_full(&self) -> bool {
        matches!(
            self,
            Self::PendingFull { .. } | Self::ReadyFull { .. } | Self::ReadyMaxed
        )
    }
}

enum TimeoutInfo {
    OutgoingConnect,

    SendConnectionMessage,
    SendMetadataMessage,
    SendAckMessage,

    ReceiveConnectionMessage,
    ReceiveMetadataMessage,
    ReceiveAckMessage,
}

impl fmt::Display for TimeoutInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Timeout - {}",
            match self {
                Self::OutgoingConnect => "outgoing connection",

                Self::SendConnectionMessage => "send ConnectionMessage",
                Self::SendMetadataMessage => "send MetadataMessage",
                Self::SendAckMessage => "send AckMessage",

                Self::ReceiveConnectionMessage => "receive ConnectionMessage",
                Self::ReceiveMetadataMessage => "receive MetadataMessage",
                Self::ReceiveAckMessage => "receive AckMessage",
            }
        )
    }
}

/// Tezedge deterministic state machine.
///
/// It's only input is [Proposal] and [Effects]. Since it's deterministic,
/// Same set of proposals and effects will lead to exact same state.
///
/// It's only output/feedback mechanism is [tla_sm::GetRequests::get_requests].
#[derive(Debug, Clone)]
pub struct TezedgeState {
    pub(crate) log: Logger,
    /// Newest time seen by the state machine (logical clock). Each
    /// proposal updates this logical time.
    pub(crate) time: SystemTime,
    pub(crate) last_periodic_react: SystemTime,
    pub(crate) config: TezedgeConfig,
    pub(crate) identity: Identity,
    pub(crate) shell_compatibility_version: ShellCompatibilityVersion,
    pub(crate) potential_peers: BTreeSet<PeerListenerAddress>,
    pub(crate) pending_peers: PendingPeers,
    pub(crate) connected_peers: ConnectedPeers,
    pub(crate) blacklisted_peers: BlacklistedPeers,
    // TODO: blacklist identities as well.
    pub(crate) p2p_state: P2pState,
    /// Currently pending requests. Completed requests is removed.
    pub(crate) requests: slab::Slab<PendingRequestState>,

    /// Main chain_id
    pub(crate) main_chain_id: ChainId,
}

impl TezedgeState {
    pub fn config(&self) -> &TezedgeConfig {
        &self.config
    }

    pub fn time(&self) -> SystemTime {
        self.time
    }

    pub fn validate_proposal<P: Proposal>(
        &mut self,
        proposal: &P,
    ) -> Result<(), InvalidProposalError> {
        self.time += proposal.time_passed();

        Ok(())
    }

    pub fn blacklisted_peers(&self) -> &BlacklistedPeers {
        &self.blacklisted_peers
    }

    pub fn request_states(&self) -> &slab::Slab<PendingRequestState> {
        &self.requests
    }

    #[inline(always)]
    pub fn pending_peers_len(&self) -> usize {
        self.pending_peers.len()
    }

    pub fn meta_msg(&self) -> MetadataMessage {
        MetadataMessage::new(self.config.disable_mempool, self.config.private_node)
    }

    pub(crate) fn accept_internal<P>(&mut self, mut proposal: P)
    where
        P: Proposal,
        Self: Acceptor<P>,
    {
        // since this is internal proposal call, set passed time to 0 (nullify),
        // to avoid increasing internal clock by extra time.
        proposal.nullify_time_passed();
        self.accept(proposal)
    }

    pub(crate) fn extend_potential_peers<I>(&mut self, peers: I)
    where
        I: IntoIterator<Item = PeerListenerAddress>,
    {
        // Return if maximum number of potential peers is already reached.
        if self.potential_peers.len() >= self.config.max_potential_peers {
            return;
        }

        let limit = self.config.max_potential_peers - self.potential_peers.len();

        let connected_peers = &self.connected_peers;
        let blacklisted_peers = &self.blacklisted_peers;
        let pending_peers = &self.pending_peers;

        self.potential_peers
            .extend(peers.into_iter().take(limit).filter(|addr| {
                !blacklisted_peers.is_address_blacklisted(&addr.into())
                    && !connected_peers.contains_address(&addr.into())
                    && !pending_peers.contains_address(&addr.into())
            }));
    }

    pub fn is_peer_connected(&self, peer: &PeerAddress) -> bool {
        self.connected_peers.contains_address(peer)
    }

    pub fn is_address_blacklisted(&self, peer: &PeerAddress) -> bool {
        self.blacklisted_peers.is_address_blacklisted(peer)
    }

    pub(crate) fn check_blacklisted_peers(&mut self) {
        self.blacklisted_peers
            .take_expired_blacklisted_peers(self.time, self.config.peer_blacklist_duration);
    }

    pub(crate) fn disconnect_peer(&mut self, address: PeerAddress) {
        self.pending_peers.remove(&address);
        self.connected_peers.remove(&address);

        self.requests.insert(PendingRequestState {
            request: PendingRequest::DisconnectPeer { peer: address },
            status: RetriableRequestState::Idle { at: self.time },
        });
    }

    pub(crate) fn blacklist_peer(&mut self, address: PeerAddress) {
        if self.config.disable_blacklist {
            slog::warn!(&self.log, "Ignored blacklist request! Disconnecting instead."; "reason" => "Blacklist disabled");
            return self.disconnect_peer(address);
        }

        let pending_peer_port = self
            .pending_peers
            .remove(&address)
            .and_then(|peer| peer.listener_port());

        let listener_port = self
            .connected_peers
            .remove(&address)
            .map(|peer| peer.listener_port())
            .or(pending_peer_port);

        self.blacklisted_peers.insert_ip(
            address.ip(),
            BlacklistedPeer {
                since: self.time,
                port: listener_port,
            },
        );
        // TODO: blacklist identity as well.

        self.requests.insert(PendingRequestState {
            request: PendingRequest::BlacklistPeer { peer: address },
            status: RetriableRequestState::Idle { at: self.time },
        });
    }

    #[inline]
    fn missing_connected_peers(&self) -> usize {
        self.config
            .max_connected_peers
            .checked_sub(self.connected_peers.len())
            .unwrap_or(0)
    }

    #[inline]
    fn missing_pending_peers(&self) -> usize {
        self.config
            .max_pending_peers
            .min(self.missing_connected_peers())
            .checked_sub(self.pending_peers.len())
            .unwrap_or(0)
    }

    pub fn stats(&self) -> TezedgeStats {
        TezedgeStats {
            time: self.time,
            last_periodic_react: self.last_periodic_react,
            potential_peers_len: self.potential_peers.len(),
            connected_peers_len: self.connected_peers.len(),
            blacklisted_peers_len: self.blacklisted_peers.len(),
            pending_peers_len: self.pending_peers_len(),
            requests_len: self.requests.len(),
        }
    }
}

impl TezedgeState {
    pub fn new<'a, Efs>(
        log: Logger,
        config: TezedgeConfig,
        identity: Identity,
        shell_compatibility_version: ShellCompatibilityVersion,
        effects: &'a mut Efs,
        initial_time: SystemTime,
        main_chain_id: ChainId,
    ) -> Self
    where
        Efs: Effects,
    {
        let periodic_react_interval = config.periodic_react_interval;
        let max_connected_peers = config.max_connected_peers;
        let max_pending_peers = config.max_pending_peers;
        let reset_quotas_interval = config.reset_quotas_interval;

        if config.disable_blacklist {
            slog::warn!(&log, "Peer Blacklist is DISABLED!");
        }

        Self {
            log: log.clone(),
            config,
            identity,
            shell_compatibility_version,
            potential_peers: BTreeSet::new(),
            pending_peers: PendingPeers::with_capacity(max_pending_peers),
            connected_peers: ConnectedPeers::new(
                log,
                Some(max_connected_peers),
                reset_quotas_interval,
            ),
            blacklisted_peers: BlacklistedPeers::new(),
            p2p_state: P2pState::Pending,
            requests: slab::Slab::new(),
            time: initial_time,
            last_periodic_react: initial_time - periodic_react_interval,
            main_chain_id,
        }
        .init(effects)
    }

    fn init<Efs: Effects>(mut self, effects: &mut Efs) -> Self {
        self.requests.insert(PendingRequestState {
            request: PendingRequest::StartListeningForNewPeers,
            status: RetriableRequestState::Idle { at: self.time },
        });
        self.adjust_p2p_state(effects);

        self
    }

    /// Take finished handshake result and create a new connected peer.
    pub(crate) fn set_peer_connected<'a, Efs: Effects>(
        &mut self,
        effects: &'a mut Efs,
        peer_address: PeerAddress,
        result: HandshakeResult,
    ) {
        // double check to make sure we don't go over the limit.
        use P2pState::*;
        self.adjust_p2p_state(effects);
        match &self.p2p_state {
            Pending | PendingFull | Ready | ReadyFull => {}
            ReadyMaxed => {
                slog::warn!(&self.log, "Blacklisting Peer"; "reason" => "Tried to connect to peer while we are maxed out on connected peers.");
                return self.blacklist_peer(peer_address);
            }
        }

        let public_key_hash = result.public_key.public_key_hash().unwrap();

        let connected_peer =
            self.connected_peers
                .set_peer_connected(self.time, peer_address, result);

        self.requests.insert(PendingRequestState {
            request: PendingRequest::NotifyHandshakeSuccessful {
                peer_address: peer_address,
                peer_public_key_hash: public_key_hash,
                metadata: MetadataMessage::new(
                    connected_peer.disable_mempool,
                    connected_peer.private_node,
                ),
                network_version: connected_peer.version.clone(),
            },
            status: RetriableRequestState::Idle { at: self.time },
        });

        // ask for more peers
        connected_peer.enqueue_send_message(PeerMessage::Bootstrap);
        // ask for current branch
        connected_peer
            .enqueue_send_message(GetCurrentBranchMessage::new(self.main_chain_id.clone()).into());
    }

    pub(crate) fn adjust_p2p_state<'a, Efs>(&mut self, effects: &'a mut Efs)
    where
        Efs: Effects,
    {
        use P2pState::*;
        let min_connected = self.config.min_connected_peers as usize;
        let missing_connected = self.missing_connected_peers();
        let missing_pending = self.missing_pending_peers();

        let mut should_initiate_connections = false;

        if missing_connected == 0 {
            self.p2p_state = ReadyMaxed;
        } else if self.connected_peers.len() < min_connected {
            if missing_pending == 0 {
                self.p2p_state = PendingFull;
            } else {
                should_initiate_connections = true;
                self.p2p_state = Pending;
            }
        } else {
            if missing_pending == 0 {
                self.p2p_state = ReadyFull;
            } else {
                self.p2p_state = Ready;
                should_initiate_connections = true;
            }
        }

        if should_initiate_connections {
            return self.initiate_handshakes(effects);
        }
    }

    pub(crate) fn check_timeouts<'a, Efs>(&mut self, effects: &'a mut Efs)
    where
        Efs: Effects,
    {
        let now = self.time;
        let peer_timeout = self.config.peer_timeout;

        self.requests.retain(|_, req| match &req.request {
            PendingRequest::ConnectPeer { .. } => match &req.status {
                RetriableRequestState::Idle { at } | RetriableRequestState::Pending { at } => {
                    // retain only requests which hasn't timed out
                    now.duration_since(*at)
                        .map(|passed| passed < peer_timeout)
                        .unwrap_or(true)
                }
                _ => false,
            },
            _ => true,
        });

        use HandshakeStep::*;
        use RequestState::*;

        let end_handshakes = self
            .pending_peers
            .iter_mut()
            .filter_map(|(_, peer)| {
                let (at, timeout_info) = match &peer.step {
                    Initiated { at } => (
                        at,
                        if peer.incoming {
                            TimeoutInfo::ReceiveConnectionMessage
                        } else {
                            TimeoutInfo::OutgoingConnect
                        },
                    ),

                    // receive timed out
                    Connect {
                        sent: Success { at, .. },
                        ..
                    } => (at, TimeoutInfo::ReceiveConnectionMessage),
                    Metadata {
                        sent: Success { at, .. },
                        ..
                    } => (at, TimeoutInfo::ReceiveMetadataMessage),
                    Ack {
                        sent: Success { at, .. },
                        ..
                    } => (at, TimeoutInfo::ReceiveAckMessage),

                    // send timed out
                    Connect {
                        sent: Idle { at, .. },
                        ..
                    }
                    | Connect {
                        sent: Pending { at, .. },
                        ..
                    } => (at, TimeoutInfo::SendConnectionMessage),

                    Metadata {
                        sent: Idle { at, .. },
                        ..
                    }
                    | Metadata {
                        sent: Pending { at, .. },
                        ..
                    } => (at, TimeoutInfo::SendMetadataMessage),

                    Ack {
                        sent: Idle { at, .. },
                        ..
                    }
                    | Ack {
                        sent: Pending { at, .. },
                        ..
                    } => (at, TimeoutInfo::SendAckMessage),
                };

                now.duration_since(*at)
                    .ok()
                    .filter(|passed| *passed >= peer_timeout)
                    .map(|_| (peer.address.clone(), timeout_info))
            })
            .collect::<Vec<_>>();

        if end_handshakes.len() == 0 {
            return;
        }

        for (peer, timeout_info) in end_handshakes.into_iter() {
            slog::warn!(&self.log, "Blacklisting peer"; "reason" => timeout_info.to_string(), "peer_address" => peer.to_string());
            self.blacklist_peer(peer);
        }
        self.initiate_handshakes(effects);
    }

    pub(crate) fn initiate_handshakes<'a, Efs>(&mut self, effects: &'a mut Efs)
    where
        Efs: Effects,
    {
        use P2pState::*;

        match self.p2p_state {
            ReadyMaxed | ReadyFull | PendingFull => return,
            Pending | Ready => {}
        }
        let len = self.potential_peers.len().min(self.missing_pending_peers());

        if len == 0 {
            return;
        }
        slog::info!(&self.log, "Initiating handshakes";
                     "connected_peers" => self.connected_peers.len(),
                     "pending_peers" => self.pending_peers.len(),
                     "potential_peers" => self.potential_peers.len(),
                     "initiated_handshakes" => len);

        let peers = effects.choose_peers_to_connect_to(&self.potential_peers, len);

        if peers.len() == 0 {
            return;
        }

        for peer in peers {
            self.potential_peers.remove(&peer);
            self.pending_peers.insert(PendingPeer::new(
                peer.into(),
                false,
                HandshakeStep::Initiated { at: self.time },
            ));
            self.requests.insert(PendingRequestState {
                status: RetriableRequestState::Idle { at: self.time },
                request: PendingRequest::ConnectPeer { peer: peer.into() },
            });
        }

        self.adjust_p2p_state(effects);
    }

    pub(crate) fn periodic_react<'a, Efs>(&mut self, effects: &'a mut Efs)
    where
        Efs: Effects,
    {
        let interval_passed = self
            .time
            .duration_since(self.last_periodic_react)
            .map(|passed| passed > self.config.periodic_react_interval)
            .unwrap_or(false);

        if interval_passed {
            self.last_periodic_react = self.time;
            self.check_timeouts(effects);
            self.check_blacklisted_peers();
            self.initiate_handshakes(effects);
        }

        self.connected_peers.periodic_react(self.time);
    }
}

#[derive(Debug, Clone)]
pub struct TezedgeStats {
    pub time: SystemTime,
    pub last_periodic_react: SystemTime,
    pub potential_peers_len: usize,
    pub connected_peers_len: usize,
    pub blacklisted_peers_len: usize,
    pub pending_peers_len: usize,
    pub requests_len: usize,
}
