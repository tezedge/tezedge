use derive_more::From;
use redux_rs::ActionId;
use serde::{Deserialize, Serialize};

use crypto::{crypto_box::PublicKey, hash::CryptoboxPublicKeyHash};
use tezos_messages::p2p::encoding::version::NetworkVersion;

use crate::Port;

use super::connection::PeerConnectionState;
use super::disconnection::PeerDisconnecting;
use super::handshaking::{PeerHandshaking, PeerHandshakingStatus};
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
    pub bytes_read: usize,
    pub bytes_written: usize,
    pub read_timestamp: ActionId,
    pub write_timestamp: ActionId,
    pub reject_read: bool,
    pub reject_write: bool,
}

impl PeerQuota {
    pub fn new(timestamp: ActionId) -> Self {
        Self {
            bytes_read: 0,
            bytes_written: 0,
            read_timestamp: timestamp,
            write_timestamp: timestamp,
            reject_read: false,
            reject_write: false,
        }
    }
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

    pub fn public_key(&self) -> Option<&[u8]> {
        match &self.status {
            PeerStatus::Potential => None,
            PeerStatus::Connecting(_) => None,
            PeerStatus::Handshaking(state) => match &state.status {
                PeerHandshakingStatus::Init { .. }
                | PeerHandshakingStatus::ConnectionMessageInit { .. }
                | PeerHandshakingStatus::ConnectionMessageEncoded { .. }
                | PeerHandshakingStatus::ConnectionMessageWritePending { .. }
                | PeerHandshakingStatus::ConnectionMessageReadPending { .. } => None,

                PeerHandshakingStatus::ConnectionMessageReady {
                    remote_message: remote_connection_message,
                    ..
                }
                | PeerHandshakingStatus::EncryptionReady {
                    remote_connection_message,
                    ..
                }
                | PeerHandshakingStatus::MetadataMessageInit {
                    remote_connection_message,
                    ..
                }
                | PeerHandshakingStatus::MetadataMessageEncoded {
                    remote_connection_message,
                    ..
                }
                | PeerHandshakingStatus::MetadataMessageWritePending {
                    remote_connection_message,
                    ..
                }
                | PeerHandshakingStatus::MetadataMessageReadPending {
                    remote_connection_message,
                    ..
                }
                | PeerHandshakingStatus::MetadataMessageReady {
                    remote_connection_message,
                    ..
                }
                | PeerHandshakingStatus::AckMessageInit {
                    remote_connection_message,
                    ..
                }
                | PeerHandshakingStatus::AckMessageEncoded {
                    remote_connection_message,
                    ..
                }
                | PeerHandshakingStatus::AckMessageWritePending {
                    remote_connection_message,
                    ..
                }
                | PeerHandshakingStatus::AckMessageReadPending {
                    remote_connection_message,
                    ..
                }
                | PeerHandshakingStatus::AckMessageReady {
                    remote_connection_message,
                    ..
                } => Some(remote_connection_message.public_key()),
            },
            PeerStatus::Handshaked(state) => Some(state.public_key.as_ref().as_ref()),
            PeerStatus::Disconnecting(_) => None,
            PeerStatus::Disconnected => None,
        }
    }
}
