// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::cmp;

use redux_rs::ActionWithId;

use crate::peer::chunk::write::{PeerChunkWrite, PeerChunkWriteState};
use crate::peer::handshaking::{PeerHandshaking, PeerHandshakingStatus};
use crate::peer::{PeerHandshaked, PeerStatus};
use crate::{Action, State};

use super::PeerBinaryMessageWriteState;

const MAX_UNENCRYPTED_CHUNK_SIZE: usize =
    tezos_messages::p2p::binary_message::CONTENT_LENGTH_MAX - crypto::crypto_box::BOX_ZERO_BYTES;

pub fn peer_binary_message_write_reducer(state: &mut State, action: &ActionWithId<Action>) {
    match &action.action {
        Action::PeerBinaryMessageWriteSetContent(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                let binary_message_state = match &mut peer.status {
                    PeerStatus::Handshaking(PeerHandshaking { status, .. }) => match status {
                        PeerHandshakingStatus::MetadataMessageWritePending {
                            binary_message_state,
                            ..
                        }
                        | PeerHandshakingStatus::AckMessageWritePending {
                            binary_message_state,
                            ..
                        } => binary_message_state,
                        _ => return,
                    },
                    PeerStatus::Handshaked(PeerHandshaked { message_write, .. }) => {
                        &mut message_write.current
                    }
                    _ => return,
                };

                if let PeerBinaryMessageWriteState::Init { crypto } = binary_message_state {
                    let next_chunk_pos = cmp::min(MAX_UNENCRYPTED_CHUNK_SIZE, action.message.len());
                    let (chunk_content, rest_of_message_content) =
                        action.message.split_at(next_chunk_pos);
                    *binary_message_state = PeerBinaryMessageWriteState::Pending {
                        chunk_content: chunk_content.to_vec(),
                        rest_of_message_content: rest_of_message_content.to_vec(),
                        chunk: PeerChunkWrite {
                            crypto: crypto.clone(),
                            state: PeerChunkWriteState::Init,
                        },
                    };
                }
            }
        }
        Action::PeerBinaryMessageWriteNextChunk(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                let binary_message_state = match &mut peer.status {
                    PeerStatus::Handshaking(PeerHandshaking { status, .. }) => match status {
                        PeerHandshakingStatus::MetadataMessageWritePending {
                            binary_message_state,
                            ..
                        }
                        | PeerHandshakingStatus::AckMessageWritePending {
                            binary_message_state,
                            ..
                        } => binary_message_state,
                        _ => return,
                    },
                    PeerStatus::Handshaked(PeerHandshaked { message_write, .. }) => {
                        &mut message_write.current
                    }
                    _ => return,
                };

                if let PeerBinaryMessageWriteState::Pending {
                    rest_of_message_content,
                    chunk: PeerChunkWrite { crypto, .. },
                    ..
                } = binary_message_state
                {
                    if !rest_of_message_content.is_empty() {
                        let next_chunk_pos =
                            cmp::min(MAX_UNENCRYPTED_CHUNK_SIZE, rest_of_message_content.len());
                        let (chunk_content, rest_of_message_content) =
                            rest_of_message_content.split_at(next_chunk_pos);
                        *binary_message_state = PeerBinaryMessageWriteState::Pending {
                            chunk_content: chunk_content.to_vec(),
                            rest_of_message_content: rest_of_message_content.to_vec(),
                            chunk: PeerChunkWrite {
                                crypto: crypto.clone(),
                                state: PeerChunkWriteState::Init,
                            },
                        };
                    } else {
                        *binary_message_state = PeerBinaryMessageWriteState::Ready {
                            crypto: crypto.clone(),
                        };
                    }
                }
            }
        }
        _ => {}
    }
}
