// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use derive_more::From;
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, time::Instant};

use crate::peer::PeerToken;

/// Event coming from `Manager`.
///
/// Each event updates internal logical clock and also triggers some actions.
#[derive(From, Serialize, Deserialize, Debug, Clone)]
pub enum Event {
    /// `mio::Waker` has been used to wake up `mio::Poll::poll`.
    Wakeup(WakeupEvent),

    /// Event for P2p listening socket.
    ///
    /// This usually means that we have incoming connections that we need
    /// to "accept" using `Manager::accept_connection`.
    P2pServer(P2pServerEvent),

    /// Event for P2p peer.
    P2pPeer(P2pPeerEvent),

    /// Event for P2p peer, which wasn't found in `MioService`. Should
    /// be impossible!
    P2pPeerUnknown(P2pPeerUnknownEvent),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WakeupEvent;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct P2pServerEvent;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct P2pPeerEvent {
    pub token: PeerToken,

    pub address: SocketAddr,

    /// Peer's stream is ready for reading.
    pub is_readable: bool,

    /// Peer's stream is ready for writing.
    pub is_writable: bool,

    /// Connection to peer has been closed.
    pub is_closed: bool,
}

impl P2pPeerEvent {
    #[inline(always)]
    pub fn token(&self) -> PeerToken {
        self.token
    }

    #[inline(always)]
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    #[inline(always)]
    pub fn is_readable(&self) -> bool {
        self.is_readable
    }

    #[inline(always)]
    pub fn is_writable(&self) -> bool {
        self.is_writable
    }

    #[inline(always)]
    pub fn is_closed(&self) -> bool {
        self.is_closed
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct P2pPeerUnknownEvent {
    pub token: PeerToken,

    /// Peer's stream is ready for reading.
    pub is_readable: bool,

    /// Peer's stream is ready for writing.
    pub is_writable: bool,

    /// Connection to peer has been closed.
    pub is_closed: bool,
}
