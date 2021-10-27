// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use redux_rs::{ActionWithId, Store};

use crate::peer::binary_message::read::PeerBinaryMessageReadState;
use crate::peer::chunk::read::{
    PeerChunkReadDecryptAction, PeerChunkReadError, PeerChunkReadErrorAction,
    PeerChunkReadReadyAction, PeerChunkReadState,
};
use crate::peer::handshaking::{PeerHandshaking, PeerHandshakingStatus};
use crate::peer::message::read::PeerMessageReadState;
use crate::peer::{PeerHandshaked, PeerStatus, PeerTryReadLoopStartAction};
use crate::peers::graylist::PeersGraylistAddressAction;
use crate::{Action, Service, State};

pub fn peer_chunk_read_effects<S>(
    store: &mut Store<State, S, Action>,
    action: &ActionWithId<Action>,
) where
    S: Service,
{
    match &action.action {
        Action::PeerChunkReadInit(action) => {
            if let Some(peer) = store.state.get().peers.get(&action.address) {
                if peer.try_read_loop.can_be_started() {
                    store.dispatch(
                        PeerTryReadLoopStartAction {
                            address: action.address,
                        }
                        .into(),
                    );
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
                                PeerChunkReadState::PendingSize { .. }
                                | PeerChunkReadState::PendingBody { .. } => {
                                    if peer.try_read_loop.can_be_started() {
                                        store.dispatch(
                                            PeerTryReadLoopStartAction {
                                                address: action.address,
                                            }
                                            .into(),
                                        );
                                    }
                                }
                                PeerChunkReadState::Ready { .. } => {
                                    store.dispatch(
                                        PeerChunkReadReadyAction {
                                            address: action.address,
                                        }
                                        .into(),
                                    );
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
                    | PeerBinaryMessageReadState::Pending { chunk, .. } => match &chunk.state {
                        PeerChunkReadState::PendingSize { .. }
                        | PeerChunkReadState::PendingBody { .. } => {
                            if peer.try_read_loop.can_be_started() {
                                store.dispatch(
                                    PeerTryReadLoopStartAction {
                                        address: action.address,
                                    }
                                    .into(),
                                );
                            }
                        }
                        PeerChunkReadState::EncryptedReady {
                            chunk_encrypted: chunk_content_encrypted,
                        } => match chunk.crypto.decrypt(&chunk_content_encrypted) {
                            Ok(decrypted_bytes) => store.dispatch(
                                PeerChunkReadDecryptAction {
                                    address: action.address,
                                    decrypted_bytes,
                                }
                                .into(),
                            ),
                            Err(err) => store.dispatch(
                                PeerChunkReadErrorAction {
                                    address: action.address,
                                    error: PeerChunkReadError::from(err),
                                }
                                .into(),
                            ),
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
                    | PeerBinaryMessageReadState::Pending { chunk, .. } => match &chunk.state {
                        PeerChunkReadState::Ready { .. } => {
                            store.dispatch(
                                PeerChunkReadReadyAction {
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
        Action::PeerChunkReadError(action) => {
            store.dispatch(
                PeersGraylistAddressAction {
                    address: action.address,
                }
                .into(),
            );
        }
        _ => {}
    }
}
