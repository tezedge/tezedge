// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use tezos_messages::p2p::binary_message::BinaryChunk;

use crate::peer::binary_message::write::PeerBinaryMessageWriteState;
use crate::peer::handshaking::{PeerHandshaking, PeerHandshakingStatus};
use crate::peer::{PeerHandshaked, PeerStatus, PeerTryWriteLoopStartAction};
use crate::peers::graylist::PeersGraylistAddressAction;
use crate::{Action, ActionWithMeta, Service, Store};

use super::{
    PeerChunkWrite, PeerChunkWriteCreateChunkAction, PeerChunkWriteEncryptContentAction,
    PeerChunkWriteError, PeerChunkWriteErrorAction, PeerChunkWriteReadyAction, PeerChunkWriteState,
};

pub fn peer_chunk_write_effects<S>(store: &mut Store<S>, action: &ActionWithMeta)
where
    S: Service,
{
    match &action.action {
        Action::PeerChunkWriteSetContent(action) => {
            if let Some(peer) = store.state.get().peers.get(&action.address) {
                let binary_message_state = match &peer.status {
                    PeerStatus::Handshaking(PeerHandshaking { status, .. }) => match status {
                        PeerHandshakingStatus::ConnectionMessageWritePending {
                            chunk_state: PeerChunkWriteState::UnencryptedContent { content },
                            ..
                        } => {
                            return match BinaryChunk::from_content(content) {
                                Ok(chunk) => {
                                    store.dispatch(PeerChunkWriteCreateChunkAction {
                                        address: action.address,
                                        chunk,
                                    });
                                }
                                Err(err) => {
                                    store.dispatch(PeerChunkWriteErrorAction {
                                        address: action.address,
                                        error: err.into(),
                                    });
                                }
                            }
                        }
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
                        &message_write.current
                    }
                    _ => return,
                };

                if let PeerBinaryMessageWriteState::Pending {
                    chunk:
                        PeerChunkWrite {
                            crypto,
                            state: PeerChunkWriteState::UnencryptedContent { content },
                        },
                    ..
                } = binary_message_state
                {
                    match crypto.encrypt(content) {
                        Ok(encrypted_content) => {
                            store.dispatch(PeerChunkWriteEncryptContentAction {
                                address: action.address,
                                encrypted_content,
                            });
                        }
                        Err(err) => {
                            store.dispatch(PeerChunkWriteErrorAction {
                                address: action.address,
                                error: PeerChunkWriteError::from(err),
                            });
                        }
                    };
                };
            }
        }
        Action::PeerChunkWriteEncryptContent(action) => {
            if let Some(peer) = store.state.get().peers.get(&action.address) {
                let binary_message_state = match &peer.status {
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
                        &message_write.current
                    }
                    _ => return,
                };

                if let PeerBinaryMessageWriteState::Pending {
                    chunk:
                        PeerChunkWrite {
                            state: PeerChunkWriteState::EncryptedContent { content },
                            ..
                        },
                    ..
                } = binary_message_state
                {
                    match BinaryChunk::from_content(content) {
                        Ok(chunk) => {
                            store.dispatch(PeerChunkWriteCreateChunkAction {
                                address: action.address,
                                chunk,
                            });
                        }
                        Err(err) => {
                            store.dispatch(PeerChunkWriteErrorAction {
                                address: action.address,
                                error: err.into(),
                            });
                        }
                    }
                };
            }
        }
        Action::PeerChunkWriteCreateChunk(action) => {
            if let Some(peer) = store.state.get().peers.get(&action.address) {
                let binary_message_state = match &peer.status {
                    PeerStatus::Handshaking(PeerHandshaking { status, .. }) => match status {
                        PeerHandshakingStatus::ConnectionMessageWritePending {
                            chunk_state: PeerChunkWriteState::Pending { .. },
                            ..
                        } => {
                            if peer.try_write_loop.can_be_started() {
                                store.dispatch(PeerTryWriteLoopStartAction {
                                    address: action.address,
                                });
                            }
                            return;
                        }
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
                        &message_write.current
                    }
                    _ => return,
                };

                if let PeerBinaryMessageWriteState::Pending {
                    chunk:
                        PeerChunkWrite {
                            state: PeerChunkWriteState::Pending { .. },
                            ..
                        },
                    ..
                } = binary_message_state
                {
                    if peer.try_write_loop.can_be_started() {
                        store.dispatch(PeerTryWriteLoopStartAction {
                            address: action.address,
                        });
                    }
                };
            }
        }
        Action::PeerChunkWritePart(action) => {
            if let Some(peer) = store.state.get().peers.get(&action.address) {
                let binary_message_state = match &peer.status {
                    PeerStatus::Handshaking(PeerHandshaking { status, .. }) => match status {
                        PeerHandshakingStatus::ConnectionMessageWritePending {
                            chunk_state,
                            ..
                        } => {
                            return match chunk_state {
                                PeerChunkWriteState::Pending { .. } => {
                                    if peer.try_write_loop.can_be_started() {
                                        store.dispatch(PeerTryWriteLoopStartAction {
                                            address: action.address,
                                        });
                                    }
                                }
                                PeerChunkWriteState::Ready { .. } => {
                                    store.dispatch(PeerChunkWriteReadyAction {
                                        address: action.address,
                                    });
                                }
                                _ => {}
                            }
                        }
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
                        &message_write.current
                    }
                    _ => return,
                };
                if let PeerBinaryMessageWriteState::Pending { chunk, .. } = binary_message_state {
                    match &chunk.state {
                        PeerChunkWriteState::Pending { .. } => {
                            if peer.try_write_loop.can_be_started() {
                                store.dispatch(PeerTryWriteLoopStartAction {
                                    address: action.address,
                                });
                            }
                        }
                        PeerChunkWriteState::Ready { .. } => {
                            store.dispatch(PeerChunkWriteReadyAction {
                                address: action.address,
                            });
                        }
                        _ => {}
                    }
                };
            }
        }
        Action::PeerChunkWriteError(action) => {
            store.dispatch(PeersGraylistAddressAction {
                address: action.address,
            });
        }
        _ => {}
    }
}
