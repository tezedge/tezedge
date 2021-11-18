// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use tezos_messages::p2p::binary_message::CONTENT_LENGTH_FIELD_BYTES;

use crate::peer::binary_message::read::PeerBinaryMessageReadState;
use crate::peer::chunk::read::PeerChunkReadState;
use crate::peer::handshaking::{PeerHandshaking, PeerHandshakingStatus};
use crate::peer::message::read::PeerMessageReadState;
use crate::peer::{PeerHandshaked, PeerStatus};
use crate::{Action, ActionWithMeta, State};

pub fn peer_chunk_read_reducer(state: &mut State, action: &ActionWithMeta) {
    match &action.action {
        Action::PeerChunkReadInit(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                let chunk_state = match &mut peer.status {
                    PeerStatus::Handshaking(PeerHandshaking { status, .. }) => match status {
                        PeerHandshakingStatus::ConnectionMessageReadPending {
                            chunk_state, ..
                        } => chunk_state,
                        PeerHandshakingStatus::MetadataMessageReadPending {
                            binary_message_state,
                            ..
                        }
                        | PeerHandshakingStatus::AckMessageReadPending {
                            binary_message_state,
                            ..
                        } => match binary_message_state {
                            PeerBinaryMessageReadState::PendingFirstChunk { chunk }
                            | PeerBinaryMessageReadState::Pending { chunk, .. } => &mut chunk.state,
                            _ => return,
                        },
                        _ => return,
                    },
                    PeerStatus::Handshaked(PeerHandshaked { message_read, .. }) => {
                        match message_read {
                            PeerMessageReadState::Pending {
                                binary_message_read,
                            } => match binary_message_read {
                                PeerBinaryMessageReadState::PendingFirstChunk { chunk }
                                | PeerBinaryMessageReadState::Pending { chunk, .. } => {
                                    &mut chunk.state
                                }
                                _ => return,
                            },
                            _ => return,
                        }
                    }
                    _ => return,
                };

                if let PeerChunkReadState::Init = chunk_state {
                    *chunk_state = PeerChunkReadState::PendingSize { buffer: Vec::new() };
                }
            }
        }
        Action::PeerChunkReadPart(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                let binary_message_state = match &mut peer.status {
                    PeerStatus::Handshaking(PeerHandshaking { status, .. }) => match status {
                        PeerHandshakingStatus::ConnectionMessageReadPending {
                            chunk_state, ..
                        } => {
                            return match chunk_state {
                                PeerChunkReadState::PendingSize { buffer } => {
                                    if buffer.len() + action.bytes.len()
                                        <= CONTENT_LENGTH_FIELD_BYTES
                                    {
                                        buffer.extend_from_slice(&action.bytes);
                                        if buffer.len() == CONTENT_LENGTH_FIELD_BYTES {
                                            let size = ((u16::from(buffer[0]) << 8)
                                                + u16::from(buffer[1]))
                                            .into();
                                            *chunk_state = PeerChunkReadState::PendingBody {
                                                buffer: Vec::new(),
                                                size,
                                            };
                                        }
                                    }
                                }
                                PeerChunkReadState::PendingBody { buffer, size } => {
                                    if buffer.len() + action.bytes.len() <= *size {
                                        buffer.extend_from_slice(&action.bytes);
                                        if buffer.len() == *size {
                                            *chunk_state = PeerChunkReadState::Ready {
                                                chunk: buffer.clone(),
                                            };
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                        PeerHandshakingStatus::MetadataMessageReadPending {
                            binary_message_state,
                            ..
                        }
                        | PeerHandshakingStatus::AckMessageReadPending {
                            binary_message_state,
                            ..
                        } => binary_message_state,
                        _ => return,
                    },
                    PeerStatus::Handshaked(PeerHandshaked { message_read, .. }) => {
                        match message_read {
                            PeerMessageReadState::Pending {
                                binary_message_read,
                            } => binary_message_read,
                            _ => return,
                        }
                    }
                    _ => return,
                };

                match binary_message_state {
                    PeerBinaryMessageReadState::PendingFirstChunk { chunk }
                    | PeerBinaryMessageReadState::Pending { chunk, .. } => match &mut chunk.state {
                        PeerChunkReadState::PendingSize { buffer } => {
                            if buffer.len() + action.bytes.len() <= CONTENT_LENGTH_FIELD_BYTES {
                                buffer.extend_from_slice(&action.bytes);
                                if buffer.len() == CONTENT_LENGTH_FIELD_BYTES {
                                    let size =
                                        ((u16::from(buffer[0]) << 8) + u16::from(buffer[1])).into();
                                    chunk.state = PeerChunkReadState::PendingBody {
                                        buffer: Vec::new(),
                                        size,
                                    };
                                }
                            }
                        }
                        PeerChunkReadState::PendingBody { buffer, size } => {
                            if buffer.len() + action.bytes.len() <= *size {
                                buffer.extend_from_slice(&action.bytes);
                                if buffer.len() == *size {
                                    chunk.state = PeerChunkReadState::EncryptedReady {
                                        chunk_encrypted: buffer.clone(),
                                    };
                                }
                            }
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
        }
        Action::PeerChunkReadDecrypt(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                let binary_message_state = match &mut peer.status {
                    PeerStatus::Handshaking(PeerHandshaking { status, .. }) => match status {
                        PeerHandshakingStatus::MetadataMessageReadPending {
                            binary_message_state,
                            ..
                        }
                        | PeerHandshakingStatus::AckMessageReadPending {
                            binary_message_state,
                            ..
                        } => binary_message_state,
                        _ => return,
                    },
                    PeerStatus::Handshaked(PeerHandshaked { message_read, .. }) => {
                        match message_read {
                            PeerMessageReadState::Pending {
                                binary_message_read,
                            } => binary_message_read,
                            _ => return,
                        }
                    }
                    _ => return,
                };
                match binary_message_state {
                    PeerBinaryMessageReadState::PendingFirstChunk { chunk }
                    | PeerBinaryMessageReadState::Pending { chunk, .. } => {
                        if let PeerChunkReadState::EncryptedReady { .. } = &chunk.state {
                            chunk.state = PeerChunkReadState::Ready {
                                chunk: action.decrypted_bytes.clone(),
                            };
                            chunk.crypto.increment_nonce();
                        }
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
}
