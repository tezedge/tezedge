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
    pub quota: PeerQuota,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerQuota {
    pub quota_bytes_read: usize,
    pub quota_read_timestamp: ActionId,
}

impl Peer {
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
