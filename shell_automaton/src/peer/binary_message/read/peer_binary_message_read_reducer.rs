// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use redux_rs::ActionWithId;

use crate::peer::chunk::read::{PeerChunkRead, PeerChunkReadState};
use crate::peer::handshaking::{PeerHandshaking, PeerHandshakingStatus};
use crate::peer::message::read::PeerMessageReadState;
use crate::peer::{PeerHandshaked, PeerStatus};
use crate::{Action, State};

use super::PeerBinaryMessageReadState;

pub fn peer_binary_message_read_reducer(state: &mut State, action: &ActionWithId<Action>) {
    match &action.action {
        Action::PeerBinaryMessageReadInit(action) => {
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
                    PeerBinaryMessageReadState::Init { crypto } => {
                        *binary_message_state = PeerBinaryMessageReadState::PendingFirstChunk {
                            chunk: PeerChunkRead {
                                crypto: crypto.clone(),
                                state: PeerChunkReadState::Init,
                            },
                        }
                    }
                    _ => {}
                }
            }
        }
        Action::PeerBinaryMessageReadSizeReady(action) => {
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
                    PeerBinaryMessageReadState::PendingFirstChunk {
                        chunk:
                            PeerChunkRead {
                                crypto,
                                state: PeerChunkReadState::Ready { chunk },
                            },
                    } => {
                        if action.size > chunk.len() {
                            *binary_message_state = PeerBinaryMessageReadState::Pending {
                                buffer: chunk.clone(),
                                size: action.size,
                                chunk: PeerChunkRead {
                                    crypto: crypto.clone(),
                                    state: PeerChunkReadState::Init,
                                },
                            };
                        } else {
                            *binary_message_state = PeerBinaryMessageReadState::Ready {
                                crypto: crypto.clone(),
                                message: chunk.clone(),
                            };
                        }
                    }
                    _ => {}
                };
            }
        }
        Action::PeerBinaryMessageReadChunkReady(action) => {
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
                    PeerBinaryMessageReadState::Pending {
                        buffer,
                        size,
                        chunk: PeerChunkRead { crypto, state },
                    } => {
                        if let PeerChunkReadState::Ready {
                            chunk: chunk_content,
                        } = state
                        {
                            if buffer.len() + chunk_content.len() <= *size {
                                buffer.extend_from_slice(&chunk_content);
                                if buffer.len() == *size {
                                    *binary_message_state = PeerBinaryMessageReadState::Ready {
                                        crypto: crypto.clone(),
                                        message: buffer.clone(),
                                    }
                                } else {
                                    *state = PeerChunkReadState::Init;
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
}
