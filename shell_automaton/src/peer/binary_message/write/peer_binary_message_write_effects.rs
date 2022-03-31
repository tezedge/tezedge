// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::peer::chunk::write::{
    PeerChunkWrite, PeerChunkWriteSetContentAction, PeerChunkWriteState,
};
use crate::peer::handshaking::{PeerHandshaking, PeerHandshakingStatus};
use crate::peer::{PeerHandshaked, PeerStatus};
use crate::peers::graylist::{PeerGraylistReason, PeersGraylistAddressAction};
use crate::{Action, ActionWithMeta, Service, Store};

use super::{
    PeerBinaryMessageWriteNextChunkAction, PeerBinaryMessageWriteReadyAction,
    PeerBinaryMessageWriteState,
};

pub fn peer_binary_message_write_effects<S>(store: &mut Store<S>, action: &ActionWithMeta)
where
    S: Service,
{
    match &action.action {
        Action::PeerBinaryMessageWriteSetContent(action) => {
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
                            state: PeerChunkWriteState::Init,
                            ..
                        },
                    chunk_content,
                    ..
                } = binary_message_state
                {
                    let content = chunk_content.clone();
                    store.dispatch(PeerChunkWriteSetContentAction {
                        address: action.address,
                        content,
                    });
                }
            }
        }
        Action::PeerChunkWriteReady(action) => {
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
                                state: PeerChunkWriteState::Ready { .. },
                                ..
                            },
                        ..
                    } => {
                        store.dispatch(PeerBinaryMessageWriteNextChunkAction {
                            address: action.address,
                        });
                    }
                    PeerBinaryMessageWriteState::Ready { .. } => {
                        store.dispatch(PeerBinaryMessageWriteReadyAction {
                            address: action.address,
                        });
                    }
                    _ => {}
                };
            }
        }
        Action::PeerBinaryMessageWriteNextChunk(action) => {
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
                    PeerBinaryMessageWriteState::Pending { chunk_content, .. } => {
                        let content = chunk_content.clone();
                        store.dispatch(PeerChunkWriteSetContentAction {
                            address: action.address,
                            content,
                        });
                    }
                    PeerBinaryMessageWriteState::Ready { .. } => {
                        store.dispatch(PeerBinaryMessageWriteReadyAction {
                            address: action.address,
                        });
                    }
                    _ => {}
                };
            }
        }
        Action::PeerBinaryMessageWriteError(action) => {
            store.dispatch(PeersGraylistAddressAction {
                address: action.address,
                reason: PeerGraylistReason::BinaryMessageWriteError,
            });
        }
        _ => {}
    }
}
