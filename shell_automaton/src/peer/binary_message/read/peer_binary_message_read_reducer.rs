// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::peer::chunk::read::{PeerChunkRead, PeerChunkReadState};
use crate::peer::handshaking::{PeerHandshaking, PeerHandshakingStatus};
use crate::peer::message::read::PeerMessageReadState;
use crate::peer::{PeerHandshaked, PeerStatus};
use crate::{Action, ActionWithMeta, State};

use super::PeerBinaryMessageReadState;

pub fn peer_binary_message_read_reducer(state: &mut State, action: &ActionWithMeta) {
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

                if let PeerBinaryMessageReadState::Init { crypto } = binary_message_state {
                    *binary_message_state = PeerBinaryMessageReadState::PendingFirstChunk {
                        chunk: PeerChunkRead {
                            crypto: crypto.clone(),
                            state: PeerChunkReadState::Init,
                        },
                    }
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

                if let PeerBinaryMessageReadState::PendingFirstChunk {
                                        chunk:
                                            PeerChunkRead {
                                                crypto,
                                                state: PeerChunkReadState::Ready { chunk },
                                            },
                                    } = binary_message_state {
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

                if let PeerBinaryMessageReadState::Pending {
                    buffer,
                    size,
                    chunk: PeerChunkRead { crypto, state },
                } = binary_message_state
                {
                    if let PeerChunkReadState::Ready {
                        chunk: chunk_content,
                    } = state
                    {
                        if buffer.len() + chunk_content.len() <= *size {
                            buffer.extend_from_slice(chunk_content);
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
            }
        }
        _ => {}
    }
}
