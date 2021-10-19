use derive_more::From;
use redux_rs::ActionId;
use serde::{Deserialize, Serialize};

use crypto::{crypto_box::PublicKey, hash::CryptoboxPublicKeyHash};
use tezos_messages::p2p::encoding::version::NetworkVersion;

use crate::Port;

use super::connection::PeerConnectionState;
use super::disconnection::PeerDisconnecting;
use super::handshaking::PeerHandshaking;
use super::message::read::PeerMessageReadState;
use super::message::write::PeerMessageWriteState;
use super::{PeerCrypto, PeerToken};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerHandshaked {
    pub token: PeerToken,
    pub port: Port,
    pub version: NetworkVersion,
    pub public_key: PublicKey,
    pub public_key_hash: CryptoboxPublicKeyHash,
    pub crypto: PeerCrypto,
    pub disable_mempool: bool,
    pub private_node: bool,

    pub message_read: PeerMessageReadState,
    pub message_write: PeerMessageWriteState,
}

#[derive(From, Serialize, Deserialize, Debug, Clone)]
pub enum PeerStatus {
    /// Peer is a potential peer.
    Potential,

    Connecting(PeerConnectionState),
    Handshaking(PeerHandshaking),
    Handshaked(PeerHandshaked),

    Disconnecting(PeerDisconnecting),
    Disconnected,
}

impl PeerStatus {
    pub fn as_handshaked(&self) -> Option<&PeerHandshaked> {
        match self {
            Self::Handshaked(v) => Some(v),
            _ => None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Peer {
    pub status: PeerStatus,
    pub read_state: PeerReadState,
    pub write_state: PeerWriteState,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PeerReadState {
    Idle {
        bytes_read: usize,
        timestamp: ActionId,
    },
    Readable {
        bytes_read: usize,
        timestamp: ActionId,
    },
    OutOfQuota {
        timestamp: ActionId,
    },
    Closed,
}

impl PeerReadState {
    pub fn new(timestamp: ActionId) -> Self {
        Self::Idle {
            bytes_read: 0,
            timestamp,
        }
    }

    pub fn last_update_timestamp(&self) -> Option<&ActionId> {
        match self {
            PeerReadState::Idle { timestamp, .. }
            | PeerReadState::Readable { timestamp, .. }
            | PeerReadState::OutOfQuota { timestamp } => Some(timestamp),
            _ => None,
        }
    }

    pub fn time_since_last_update(&self, now: &ActionId) -> Option<u128> {
        self.last_update_timestamp()
            .map(|timestamp| now.duration_since(*timestamp).as_millis())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PeerWriteState {
    Idle {
        bytes_written: usize,
        timestamp: ActionId,
    },
    Writable {
        bytes_written: usize,
        timestamp: ActionId,
    },
    OutOfQuota {
        timestamp: ActionId,
    },
    Closed,
}

impl PeerWriteState {
    pub fn new(timestamp: ActionId) -> Self {
        Self::Idle {
            bytes_written: 0,
            timestamp,
        }
    }

    pub fn last_update_timestamp(&self) -> Option<&ActionId> {
        match self {
            PeerWriteState::Idle { timestamp, .. }
            | PeerWriteState::Writable { timestamp, .. }
            | PeerWriteState::OutOfQuota { timestamp } => Some(timestamp),
            _ => None,
        }
    }

    pub fn time_since_last_update(&self, now: &ActionId) -> Option<u128> {
        self.last_update_timestamp()
            .map(|timestamp| now.duration_since(*timestamp).as_millis())
    }
}

impl Peer {
    pub fn new(status: PeerStatus, timestamp: ActionId) -> Self {
        Self {
            status,
            read_state: PeerReadState::new(timestamp),
            write_state: PeerWriteState::new(timestamp),
        }
    }

    pub fn token(&self) -> Option<PeerToken> {
        match &self.status {
            PeerStatus::Potential => None,
            PeerStatus::Connecting(state) => state.token(),
            PeerStatus::Handshaking(state) => Some(state.token),
            PeerStatus::Handshaked(state) => Some(state.token),
            PeerStatus::Disconnecting(state) => Some(state.token),
            PeerStatus::Disconnected => None,
        }
    }
}
