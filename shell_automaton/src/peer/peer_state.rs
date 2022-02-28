// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use derive_more::From;
use serde::{Deserialize, Serialize};

use crypto::crypto_box::PublicKey;
use crypto::hash::CryptoboxPublicKeyHash;
use tezos_messages::p2p::encoding::version::NetworkVersion;

use crate::{ActionId, Port};

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

    /// Level of the current head received from peer.
    pub current_head_level: Option<i32>,
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

    pub fn as_handshaked_mut(&mut self) -> Option<&mut PeerHandshaked> {
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
    pub try_read_loop: PeerIOLoopState,
    pub try_write_loop: PeerIOLoopState,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PeerIOLoopState {
    Idle,
    Started { time: u64 },
    Finished { time: u64, result: PeerIOLoopResult },
}

impl PeerIOLoopState {
    pub fn can_be_started(&self) -> bool {
        match self {
            Self::Idle => true,
            Self::Started { .. } => false,
            Self::Finished { result, .. } => match result {
                PeerIOLoopResult::NotReady => true,
                PeerIOLoopResult::FullyConsumed => false,
                PeerIOLoopResult::ByteQuotaReached => false,
                PeerIOLoopResult::MaxIOSyscallBoundReached => false,
            },
        }
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PeerIOLoopResult {
    /// We aren't ready for making more progress, even though resource
    /// might not be fully consumed.
    NotReady,

    /// We fully consumed available resource.
    FullyConsumed,

    /// We reached the limit on maximum transfer for some interval.
    ByteQuotaReached,

    /// We reached the limit on max io syscalls for this iteration/loop.
    MaxIOSyscallBoundReached,
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
    /// Whether or not peer is connected on socket level.
    pub fn is_connected(&self) -> bool {
        self.token().is_some()
    }

    /// Whether or not peer is in `PeerStatus::Disconnected` state.
    pub fn is_disconnected(&self) -> bool {
        matches!(self.status, PeerStatus::Disconnected)
    }

    /// Whether or not peer is handshaked (handshaking is finished successfuly).
    pub fn is_handshaked(&self) -> bool {
        matches!(self.status, PeerStatus::Handshaked(_))
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

    pub fn public_key_hash(&self) -> Option<&CryptoboxPublicKeyHash> {
        match &self.status {
            PeerStatus::Handshaked(peer) => Some(&peer.public_key_hash),
            _ => None,
        }
    }
}
