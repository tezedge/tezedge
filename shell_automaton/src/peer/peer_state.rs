use derive_more::From;
use serde::{Deserialize, Serialize};

use crypto::crypto_box::PublicKey;
use tezos_messages::p2p::encoding::version::NetworkVersion;

use crate::Port;

use super::{
    connection::PeerConnectionState, disconnection::PeerDisconnecting,
    handshaking::PeerHandshaking, PeerCrypto, PeerToken,
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerHandshaked {
    pub token: PeerToken,
    pub port: Port,
    pub version: NetworkVersion,
    pub public_key: PublicKey,
    pub crypto: PeerCrypto,
    pub disable_mempool: bool,
    pub private_node: bool,
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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Peer {
    pub status: PeerStatus,
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
