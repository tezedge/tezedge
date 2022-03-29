// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::peer::binary_message::read::PeerBinaryMessageReadState;
use crate::peer::chunk::read::{
    PeerChunkReadDecryptAction, PeerChunkReadError, PeerChunkReadErrorAction,
    PeerChunkReadReadyAction, PeerChunkReadState,
};
use crate::peer::handshaking::{PeerHandshaking, PeerHandshakingStatus};
use crate::peer::message::read::PeerMessageReadState;
use crate::peer::{PeerHandshaked, PeerStatus, PeerTryReadLoopStartAction};
use crate::peers::graylist::{PeerGraylistReason, PeersGraylistAddressAction};
use crate::{Action, ActionWithMeta, Service, Store};

pub fn peer_chunk_read_effects<S>(store: &mut Store<S>, action: &ActionWithMeta)
where
    S: Service,
{
    match &action.action {
        Action::PeerChunkReadInit(action) => {
            if let Some(peer) = store.state.get().peers.get(&action.address) {
                if peer.try_read_loop.can_be_started() {
                    store.dispatch(PeerTryReadLoopStartAction {
                        address: action.address,
                    });
                }
            }
        }
        Action::PeerChunkReadPart(action) => {
            if let Some(peer) = store.state.get().peers.get(&action.address) {
                let binary_message_state = match &peer.status {
                    PeerStatus::Handshaking(PeerHandshaking { status, .. }) => match status {
                        PeerHandshakingStatus::ConnectionMessageReadPending {
                            chunk_state, ..
                        } => {
                            return match chunk_state {
                                PeerChunkReadState::PendingBody { size, .. } if *size == 0 => {
                                    store.dispatch(PeerChunkReadErrorAction {
                                        address: action.address,
                                        error: PeerChunkReadError::ZeroSize,
                                    });
                                }
                                PeerChunkReadState::PendingSize { .. }
                                | PeerChunkReadState::PendingBody { .. } => {
                                    if peer.try_read_loop.can_be_started() {
                                        store.dispatch(PeerTryReadLoopStartAction {
                                            address: action.address,
                                        });
                                    }
                                }
                                PeerChunkReadState::Ready { .. } => {
                                    store.dispatch(PeerChunkReadReadyAction {
                                        address: action.address,
                                    });
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
                    PeerStatus::Handshaked(PeerHandshaked {
                        message_read:
                            PeerMessageReadState::Pending {
                                binary_message_read,
                            },
                        ..
                    }) => binary_message_read,

                    _ => return,
                };

                match binary_message_state {
                    PeerBinaryMessageReadState::PendingFirstChunk { chunk }
                    | PeerBinaryMessageReadState::Pending { chunk, .. } => match &chunk.state {
                        PeerChunkReadState::PendingSize { .. }
                        | PeerChunkReadState::PendingBody { .. } => {
                            if peer.try_read_loop.can_be_started() {
                                store.dispatch(PeerTryReadLoopStartAction {
                                    address: action.address,
                                });
                            }
                        }
                        PeerChunkReadState::EncryptedReady {
                            chunk_encrypted: chunk_content_encrypted,
                        } => match chunk.crypto.decrypt(chunk_content_encrypted) {
                            Ok(decrypted_bytes) => {
                                store.dispatch(PeerChunkReadDecryptAction {
                                    address: action.address,
                                    decrypted_bytes,
                                });
                            }
                            Err(err) => {
                                store.dispatch(PeerChunkReadErrorAction {
                                    address: action.address,
                                    error: PeerChunkReadError::from(err),
                                });
                            }
                        },
                        _ => {}
                    },
                    _ => {}
                };
            }
        }
        Action::PeerChunkReadDecrypt(action) => {
            if let Some(peer) = store.state.get().peers.get(&action.address) {
                let binary_message_state = match &peer.status {
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
                    PeerStatus::Handshaked(PeerHandshaked {
                        message_read:
                            PeerMessageReadState::Pending {
                                binary_message_read,
                            },
                        ..
                    }) => binary_message_read,
                    _ => return,
                };
                match binary_message_state {
                    PeerBinaryMessageReadState::PendingFirstChunk { chunk }
                    | PeerBinaryMessageReadState::Pending { chunk, .. } => {
                        if let PeerChunkReadState::Ready { .. } = &chunk.state {
                            store.dispatch(PeerChunkReadReadyAction {
                                address: action.address,
                            });
                        }
                    }
                    _ => {}
                };
            }
        }
        Action::PeerChunkReadError(action) => {
            store.dispatch(PeersGraylistAddressAction {
                address: action.address,
                reason: PeerGraylistReason::ChunkReadError,
            });
        }
        _ => {}
    }
}
