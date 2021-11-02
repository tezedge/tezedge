// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use redux_rs::{ActionWithId, Store};
use tezos_messages::p2p::binary_message::BinaryChunk;

use crate::peer::binary_message::write::PeerBinaryMessageWriteState;
use crate::peer::handshaking::{PeerHandshaking, PeerHandshakingStatus};
use crate::peer::{PeerHandshaked, PeerStatus, PeerTryWriteLoopStartAction};
use crate::peers::graylist::PeersGraylistAddressAction;
use crate::{Action, Service, State};

use super::{
    PeerChunkWrite, PeerChunkWriteCreateChunkAction, PeerChunkWriteEncryptContentAction,
    PeerChunkWriteError, PeerChunkWriteErrorAction, PeerChunkWriteReadyAction, PeerChunkWriteState,
};

pub fn peer_chunk_write_effects<S>(
    store: &mut Store<State, S, Action>,
    action: &ActionWithId<Action>,
) where
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
                            return match BinaryChunk::from_content(&content) {
                                Ok(chunk) => store.dispatch(
                                    PeerChunkWriteCreateChunkAction {
                                        address: action.address,
                                        chunk,
                                    }
                                    .into(),
                                ),
                                Err(err) => store.dispatch(
                                    PeerChunkWriteErrorAction {
                                        address: action.address,
                                        error: err.into(),
                                    }
                                    .into(),
                                ),
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

                match binary_message_state {
                    PeerBinaryMessageWriteState::Pending {
                        chunk:
                            PeerChunkWrite {
                                crypto,
                                state: PeerChunkWriteState::UnencryptedContent { content },
                            },
                        ..
                    } => {
                        match crypto.encrypt(&content) {
                            Ok(encrypted_content) => store.dispatch(
                                PeerChunkWriteEncryptContentAction {
                                    address: action.address,
                                    encrypted_content,
                                }
                                .into(),
                            ),
                            Err(err) => store.dispatch(
                                PeerChunkWriteErrorAction {
                                    address: action.address,
                                    error: PeerChunkWriteError::from(err),
                                }
                                .into(),
                            ),
                        };
                    }
                    _ => return,
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

                match binary_message_state {
                    PeerBinaryMessageWriteState::Pending {
                        chunk:
                            PeerChunkWrite {
                                state: PeerChunkWriteState::EncryptedContent { content },
                                ..
                            },
                        ..
                    } => match BinaryChunk::from_content(&content) {
                        Ok(chunk) => store.dispatch(
                            PeerChunkWriteCreateChunkAction {
                                address: action.address,
                                chunk,
                            }
                            .into(),
                        ),
                        Err(err) => store.dispatch(
                            PeerChunkWriteErrorAction {
                                address: action.address,
                                error: err.into(),
                            }
                            .into(),
                        ),
                    },
                    _ => {}
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
                                store.dispatch(
                                    PeerTryWriteLoopStartAction {
                                        address: action.address,
                                    }
                                    .into(),
                                );
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

                match binary_message_state {
                    PeerBinaryMessageWriteState::Pending {
                        chunk:
                            PeerChunkWrite {
                                state: PeerChunkWriteState::Pending { .. },
                                ..
                            },
                        ..
                    } => {
                        if peer.try_write_loop.can_be_started() {
                            store.dispatch(
                                PeerTryWriteLoopStartAction {
                                    address: action.address,
                                }
                                .into(),
                            );
                        }
                    }
                    _ => {}
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
                                        store.dispatch(
                                            PeerTryWriteLoopStartAction {
                                                address: action.address,
                                            }
                                            .into(),
                                        );
                                    }
                                }
                                PeerChunkWriteState::Ready { .. } => {
                                    store.dispatch(
                                        PeerChunkWriteReadyAction {
                                            address: action.address,
                                        }
                                        .into(),
                                    );
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
                match binary_message_state {
                    PeerBinaryMessageWriteState::Pending { chunk, .. } => match &chunk.state {
                        PeerChunkWriteState::Pending { .. } => {
                            if peer.try_write_loop.can_be_started() {
                                store.dispatch(
                                    PeerTryWriteLoopStartAction {
                                        address: action.address,
                                    }
                                    .into(),
                                );
                            }
                        }
                        PeerChunkWriteState::Ready { .. } => {
                            store.dispatch(
                                PeerChunkWriteReadyAction {
                                    address: action.address,
                                }
                                .into(),
                            );
                        }
                        _ => {}
                    },
                    _ => {}
                };
            }
        }
        Action::PeerChunkWriteError(action) => store.dispatch(
            PeersGraylistAddressAction {
                address: action.address,
            }
            .into(),
        ),
        _ => {}
    }
}
