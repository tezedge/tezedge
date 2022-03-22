// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::VecDeque;

use crypto::crypto_box::{CryptoKey, PublicKey};
use tezos_messages::p2p::encoding::ack::{AckMessage, NackMotive};

use crate::peer::binary_message::read::PeerBinaryMessageReadState;
use crate::peer::binary_message::write::PeerBinaryMessageWriteState;
use crate::peer::chunk::read::PeerChunkReadState;
use crate::peer::chunk::write::PeerChunkWriteState;
use crate::peer::connection::incoming::PeerConnectionIncomingState;
use crate::peer::connection::outgoing::PeerConnectionOutgoingState;
use crate::peer::connection::PeerConnectionState;
use crate::peer::message::read::PeerMessageReadState;
use crate::peer::message::write::PeerMessageWriteState;
use crate::peer::{PeerCrypto, PeerHandshaked, PeerStatus};
use crate::{Action, ActionWithMeta, State};

use super::{PeerHandshaking, PeerHandshakingStatus};

pub fn peer_handshaking_reducer(state: &mut State, action: &ActionWithMeta) {
    let action_time = action.time_as_nanos();

    match &action.action {
        Action::PeerHandshakingInit(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                match peer.status {
                    PeerStatus::Connecting(PeerConnectionState::Outgoing(
                        PeerConnectionOutgoingState::Success { token, .. },
                    )) => {
                        peer.status = PeerStatus::Handshaking(PeerHandshaking {
                            token,
                            incoming: false,
                            status: PeerHandshakingStatus::Init { time: action_time },
                            nack_motive: None,
                            since: action_time,
                        });
                    }
                    PeerStatus::Connecting(PeerConnectionState::Incoming(
                        PeerConnectionIncomingState::Success { token, .. },
                    )) => {
                        peer.status = PeerStatus::Handshaking(PeerHandshaking {
                            token,
                            incoming: true,
                            status: PeerHandshakingStatus::Init { time: action_time },
                            nack_motive: None,
                            since: action_time,
                        });
                    }
                    _ => {}
                }
            };
        }
        Action::PeerHandshakingConnectionMessageInit(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                if let PeerStatus::Handshaking(PeerHandshaking { status, .. }) = &mut peer.status {
                    if let PeerHandshakingStatus::Init { .. } = status {
                        *status = PeerHandshakingStatus::ConnectionMessageInit {
                            time: action_time,
                            message: action.message.clone(),
                        }
                    }
                }
            }
        }
        Action::PeerHandshakingConnectionMessageEncode(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                if let PeerStatus::Handshaking(PeerHandshaking { status, .. }) = &mut peer.status {
                    if let PeerHandshakingStatus::ConnectionMessageInit { .. } = status {
                        *status = PeerHandshakingStatus::ConnectionMessageEncoded {
                            time: action_time,
                            binary_message: action.binary_message.clone(),
                        }
                    }
                }
            }
        }
        Action::PeerHandshakingConnectionMessageWrite(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                if let PeerStatus::Handshaking(PeerHandshaking { ref mut status, .. }) = peer.status
                {
                    if let PeerHandshakingStatus::ConnectionMessageEncoded { .. } = status {
                        *status = PeerHandshakingStatus::ConnectionMessageWritePending {
                            time: action_time,
                            local_chunk: action.chunk.clone(),
                            chunk_state: PeerChunkWriteState::Init,
                        }
                    }
                }
            }
        }
        Action::PeerHandshakingConnectionMessageRead(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                if let PeerStatus::Handshaking(PeerHandshaking { ref mut status, .. }) = peer.status
                {
                    if let PeerHandshakingStatus::ConnectionMessageWritePending {
                        local_chunk,
                        ..
                    } = status
                    {
                        *status = PeerHandshakingStatus::ConnectionMessageReadPending {
                            time: action_time,
                            local_chunk: local_chunk.clone(),
                            chunk_state: PeerChunkReadState::Init,
                        }
                    }
                }
            }
        }
        Action::PeerHandshakingConnectionMessageDecode(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                if let PeerStatus::Handshaking(PeerHandshaking { ref mut status, .. }) = peer.status
                {
                    if let PeerHandshakingStatus::ConnectionMessageReadPending {
                        local_chunk,
                        chunk_state: PeerChunkReadState::Ready { .. },
                        ..
                    } = status
                    {
                        *status = PeerHandshakingStatus::ConnectionMessageReady {
                            time: action_time,
                            local_chunk: local_chunk.clone(),
                            remote_chunk: action.remote_chunk.clone(),
                            remote_message: action.message.clone(),
                        }
                    }
                }
            }
        }

        Action::PeerHandshakingEncryptionInit(action) => {
            let (token, pub_key) = match state.peers.get(&action.address) {
                Some(peer) => (peer.token(), peer.public_key()),
                None => return,
            };

            let already_connected = state
                .peers
                .iter()
                .filter(|(_, peer)| peer.token().ne(&token))
                .any(|(_, peer)| peer.public_key().eq(&pub_key));

            let connecting_to_self =
                pub_key == Some(state.config.identity.public_key.as_ref().as_ref());

            if let Some(peer) = state.peers.get_mut(&action.address) {
                if let PeerStatus::Handshaking(PeerHandshaking {
                    status,
                    nack_motive,
                    ..
                }) = &mut peer.status
                {
                    if let PeerHandshakingStatus::ConnectionMessageReady {
                        remote_message, ..
                    } = status
                    {
                        if connecting_to_self || already_connected {
                            *nack_motive = Some(NackMotive::AlreadyConnected);
                        }

                        let compatible_version = match state
                            .config
                            .shell_compatibility_version
                            .choose_compatible_version(remote_message.version())
                        {
                            Ok(compatible_version) => Some(compatible_version),
                            Err(motive) => {
                                *nack_motive = Some(motive);
                                None
                            }
                        };

                        *status = PeerHandshakingStatus::EncryptionReady {
                            time: action_time,
                            crypto: action.crypto.clone(),
                            compatible_version,
                            remote_connection_message: remote_message.clone(),
                        };
                    }
                }
            }
        }

        /////////////////// metadata exchange
        Action::PeerHandshakingMetadataMessageInit(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                if let PeerStatus::Handshaking(PeerHandshaking { status, .. }) = &mut peer.status {
                    if let PeerHandshakingStatus::EncryptionReady {
                        crypto,
                        compatible_version,
                        remote_connection_message,
                        ..
                    } = status
                    {
                        *status = PeerHandshakingStatus::MetadataMessageInit {
                            time: action_time,
                            message: action.message.clone(),
                            crypto: crypto.clone(),
                            compatible_version: compatible_version.clone(),
                            remote_connection_message: remote_connection_message.clone(),
                        }
                    }
                }
            }
        }

        Action::PeerHandshakingMetadataMessageEncode(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                if let PeerStatus::Handshaking(PeerHandshaking { status, .. }) = &mut peer.status {
                    if let PeerHandshakingStatus::MetadataMessageInit {
                        crypto,
                        compatible_version,
                        remote_connection_message,
                        ..
                    } = status
                    {
                        *status = PeerHandshakingStatus::MetadataMessageEncoded {
                            time: action_time,
                            binary_message: action.binary_message.clone(),
                            crypto: crypto.clone(),
                            compatible_version: compatible_version.clone(),
                            remote_connection_message: remote_connection_message.clone(),
                        }
                    }
                }
            }
        }

        Action::PeerHandshakingMetadataMessageWrite(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                if let PeerStatus::Handshaking(PeerHandshaking { status, .. }) = &mut peer.status {
                    if let PeerHandshakingStatus::MetadataMessageEncoded {
                        binary_message,
                        crypto,
                        compatible_version,
                        remote_connection_message,
                        ..
                    } = status
                    {
                        let (crypto, remote_nonce) = crypto.clone().split_for_writing();
                        *status = PeerHandshakingStatus::MetadataMessageWritePending {
                            time: action_time,
                            binary_message: binary_message.clone(),
                            binary_message_state: PeerBinaryMessageWriteState::Init { crypto },
                            remote_nonce,
                            compatible_version: compatible_version.clone(),
                            remote_connection_message: remote_connection_message.clone(),
                        }
                    }
                }
            }
        }

        Action::PeerHandshakingMetadataMessageRead(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                if let PeerStatus::Handshaking(PeerHandshaking { status, .. }) = &mut peer.status {
                    if let PeerHandshakingStatus::MetadataMessageWritePending {
                        binary_message_state: PeerBinaryMessageWriteState::Ready { crypto },
                        remote_nonce,
                        compatible_version,
                        remote_connection_message,
                        ..
                    } = status
                    {
                        let (crypto, local_nonce) =
                            PeerCrypto::unsplit_after_writing(crypto.clone(), remote_nonce.clone())
                                .split_for_reading();
                        *status = PeerHandshakingStatus::MetadataMessageReadPending {
                            time: action_time,
                            binary_message_state: PeerBinaryMessageReadState::Init { crypto },
                            local_nonce,
                            compatible_version: compatible_version.clone(),
                            remote_connection_message: remote_connection_message.clone(),
                        }
                    }
                }
            }
        }

        Action::PeerHandshakingMetadataMessageDecode(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                if let PeerStatus::Handshaking(PeerHandshaking { status, .. }) = &mut peer.status {
                    if let PeerHandshakingStatus::MetadataMessageReadPending {
                        binary_message_state: PeerBinaryMessageReadState::Ready { crypto, .. },
                        local_nonce,
                        compatible_version,
                        remote_connection_message,
                        ..
                    } = status
                    {
                        let crypto =
                            PeerCrypto::unsplit_after_reading(crypto.clone(), local_nonce.clone());
                        *status = PeerHandshakingStatus::MetadataMessageReady {
                            time: action_time,
                            remote_message: action.message.clone(),
                            crypto,
                            compatible_version: compatible_version.clone(),
                            remote_connection_message: remote_connection_message.clone(),
                        }
                    }
                }
            }
        }

        /////////////////// ack exchange
        Action::PeerHandshakingAckMessageInit(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                if let PeerStatus::Handshaking(PeerHandshaking { status, .. }) = &mut peer.status {
                    if let PeerHandshakingStatus::MetadataMessageReady {
                        remote_message,
                        crypto,
                        compatible_version,
                        remote_connection_message,
                        ..
                    } = status
                    {
                        *status = PeerHandshakingStatus::AckMessageInit {
                            time: action_time,
                            message: action.message.clone(),
                            crypto: crypto.clone(),
                            compatible_version: compatible_version.clone(),
                            remote_connection_message: remote_connection_message.clone(),
                            remote_metadata_message: remote_message.clone(),
                        }
                    }
                }
            }
        }

        Action::PeerHandshakingAckMessageEncode(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                if let PeerStatus::Handshaking(PeerHandshaking { status, .. }) = &mut peer.status {
                    if let PeerHandshakingStatus::AckMessageInit {
                        crypto,
                        compatible_version,
                        remote_connection_message,
                        remote_metadata_message,
                        ..
                    } = status
                    {
                        *status = PeerHandshakingStatus::AckMessageEncoded {
                            time: action_time,
                            binary_message: action.binary_message.clone(),
                            crypto: crypto.clone(),
                            compatible_version: compatible_version.clone(),
                            remote_connection_message: remote_connection_message.clone(),
                            remote_metadata_message: remote_metadata_message.clone(),
                        }
                    }
                }
            }
        }

        Action::PeerHandshakingAckMessageWrite(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                if let PeerStatus::Handshaking(PeerHandshaking { status, .. }) = &mut peer.status {
                    if let PeerHandshakingStatus::AckMessageEncoded {
                        binary_message,
                        crypto,
                        compatible_version,
                        remote_connection_message,
                        remote_metadata_message,
                        ..
                    } = status
                    {
                        let (crypto, remote_nonce) = crypto.clone().split_for_writing();
                        *status = PeerHandshakingStatus::AckMessageWritePending {
                            time: action_time,
                            binary_message: binary_message.clone(),
                            binary_message_state: PeerBinaryMessageWriteState::Init { crypto },
                            remote_nonce,
                            compatible_version: compatible_version.clone(),
                            remote_connection_message: remote_connection_message.clone(),
                            remote_metadata_message: remote_metadata_message.clone(),
                        }
                    }
                }
            }
        }

        Action::PeerHandshakingAckMessageRead(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                if let PeerStatus::Handshaking(PeerHandshaking { status, .. }) = &mut peer.status {
                    if let PeerHandshakingStatus::AckMessageWritePending {
                        binary_message_state: PeerBinaryMessageWriteState::Ready { crypto },
                        remote_nonce,
                        compatible_version,
                        remote_connection_message,
                        remote_metadata_message,
                        ..
                    } = status
                    {
                        let (crypto, local_nonce) =
                            PeerCrypto::unsplit_after_writing(crypto.clone(), remote_nonce.clone())
                                .split_for_reading();
                        *status = PeerHandshakingStatus::AckMessageReadPending {
                            time: action_time,
                            binary_message_state: PeerBinaryMessageReadState::Init { crypto },
                            local_nonce,
                            compatible_version: compatible_version.clone(),
                            remote_connection_message: remote_connection_message.clone(),
                            remote_metadata_message: remote_metadata_message.clone(),
                        }
                    }
                }
            }
        }

        Action::PeerHandshakingAckMessageDecode(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                if let PeerStatus::Handshaking(PeerHandshaking { status, .. }) = &mut peer.status {
                    if let PeerHandshakingStatus::AckMessageReadPending {
                        binary_message_state: PeerBinaryMessageReadState::Ready { crypto, .. },
                        local_nonce,
                        compatible_version,
                        remote_connection_message,
                        remote_metadata_message,
                        ..
                    } = status
                    {
                        let crypto =
                            PeerCrypto::unsplit_after_reading(crypto.clone(), local_nonce.clone());
                        *status = PeerHandshakingStatus::AckMessageReady {
                            time: action_time,
                            remote_message: action.message.clone(),
                            crypto,
                            compatible_version: compatible_version.clone(),
                            remote_connection_message: remote_connection_message.clone(),
                            remote_metadata_message: remote_metadata_message.clone(),
                        }
                    }
                }
            }
        }

        Action::PeerHandshakingFinish(content) => {
            if let Some(peer) = state.peers.get_mut(&content.address) {
                if let PeerStatus::Handshaking(PeerHandshaking {
                    status,
                    token,
                    nack_motive,
                    ..
                }) = &mut peer.status
                {
                    if nack_motive.is_some() {
                        return;
                    }

                    if let PeerHandshakingStatus::AckMessageReady {
                        remote_message,
                        crypto,
                        compatible_version,
                        remote_connection_message,
                        remote_metadata_message,
                        ..
                    } = status
                    {
                        match remote_message {
                            AckMessage::Ack => {}
                            _ => return,
                        }
                        let version = match compatible_version {
                            Some(version) => version.clone(),
                            None => return,
                        };
                        // TODO: needs to happen as soon as we receive
                        // the connection message.
                        let public_key =
                            match PublicKey::from_bytes(remote_connection_message.public_key()) {
                                Ok(v) => v,
                                Err(_) => return,
                            };
                        let public_key_hash = match public_key.public_key_hash() {
                            Ok(v) => v,
                            Err(_) => return,
                        };

                        let (read_crypto, write_crypto) = crypto.clone().split();

                        peer.status = PeerStatus::Handshaked(PeerHandshaked {
                            handshaked_since: action.time_as_nanos(),

                            token: *token,
                            port: remote_connection_message.port,
                            version,
                            public_key,
                            public_key_hash,
                            crypto: crypto.clone(),
                            disable_mempool: remote_metadata_message.disable_mempool(),
                            private_node: remote_metadata_message.private_node(),
                            message_read: PeerMessageReadState::Pending {
                                binary_message_read: PeerBinaryMessageReadState::Init {
                                    crypto: read_crypto,
                                },
                            },
                            message_write: PeerMessageWriteState {
                                queue: VecDeque::new(),
                                current: PeerBinaryMessageWriteState::Init {
                                    crypto: write_crypto,
                                },
                            },
                            remote_requests: Default::default(),
                            current_head: None,
                            current_head_last_update: None,
                        });
                    }
                }
            }
        }

        _ => {}
    }
}
