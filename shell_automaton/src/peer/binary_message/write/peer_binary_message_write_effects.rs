use redux_rs::{ActionWithId, Store};

use crate::{
    action::Action,
    peer::{
        chunk::write::{
            peer_chunk_write_state::{PeerChunkWrite, PeerChunkWriteState},
            PeerChunkWriteSetContentAction,
        },
        handshaking::{PeerHandshaking, PeerHandshakingStatus},
        PeerStatus,
    },
    service::Service,
    State,
};

use super::{
    peer_binary_message_write_actions::{
        PeerBinaryMessageWriteNextChunkAction, PeerBinaryMessageWriteReadyAction,
    },
    peer_binary_message_write_state::PeerBinaryMessageWriteState,
};

pub fn peer_binary_message_write_effects<S>(
    store: &mut Store<State, S, Action>,
    action: &ActionWithId<Action>,
) where
    S: Service,
{
    match &action.action {
        Action::PeerBinaryMessageWriteSetContent(action) => {
            if let Some(peer) = store.state.get().peers.get(&action.address) {
                match &peer.status {
                    PeerStatus::Handshaking(PeerHandshaking { status, .. }) => match status {
                        PeerHandshakingStatus::MetadataMessageWritePending {
                            binary_message_state,
                            ..
                        }
                        | PeerHandshakingStatus::AckMessageWritePending {
                            binary_message_state,
                            ..
                        } => match binary_message_state {
                            PeerBinaryMessageWriteState::Pending {
                                chunk:
                                    PeerChunkWrite {
                                        state: PeerChunkWriteState::Init,
                                        ..
                                    },
                                chunk_content,
                                ..
                            } => {
                                let content = chunk_content.clone();
                                store.dispatch(
                                    PeerChunkWriteSetContentAction {
                                        address: action.address,
                                        content,
                                    }
                                    .into(),
                                )
                            }
                            _ => {}
                        },
                        _ => {}
                    },
                    _ => {}
                }
            }
        }
        Action::PeerChunkWriteReady(action) => {
            if let Some(peer) = store.state.get().peers.get(&action.address) {
                match &peer.status {
                    PeerStatus::Handshaking(PeerHandshaking { status, .. }) => match status {
                        PeerHandshakingStatus::MetadataMessageWritePending {
                            binary_message_state,
                            ..
                        }
                        | PeerHandshakingStatus::AckMessageWritePending {
                            binary_message_state,
                            ..
                        } => match binary_message_state {
                            PeerBinaryMessageWriteState::Pending {
                                chunk:
                                    PeerChunkWrite {
                                        state: PeerChunkWriteState::Ready { .. },
                                        ..
                                    },
                                ..
                            } => store.dispatch(
                                PeerBinaryMessageWriteNextChunkAction {
                                    address: action.address,
                                }
                                .into(),
                            ),
                            PeerBinaryMessageWriteState::Ready { .. } => store.dispatch(
                                PeerBinaryMessageWriteReadyAction {
                                    address: action.address,
                                }
                                .into(),
                            ),
                            _ => {}
                        },
                        _ => {}
                    },
                    _ => {}
                }
            }
        }
        Action::PeerBinaryMessageWriteNextChunk(action) => {
            if let Some(peer) = store.state.get().peers.get(&action.address) {
                match &peer.status {
                    PeerStatus::Handshaking(PeerHandshaking { status, .. }) => match status {
                        PeerHandshakingStatus::MetadataMessageWritePending {
                            binary_message_state,
                            ..
                        }
                        | PeerHandshakingStatus::AckMessageWritePending {
                            binary_message_state,
                            ..
                        } => match binary_message_state {
                            PeerBinaryMessageWriteState::Pending { chunk_content, .. } => {
                                let content = chunk_content.clone();
                                store.dispatch(
                                    PeerChunkWriteSetContentAction {
                                        address: action.address,
                                        content,
                                    }
                                    .into(),
                                )
                            }
                            PeerBinaryMessageWriteState::Ready { .. } => store.dispatch(
                                PeerBinaryMessageWriteReadyAction {
                                    address: action.address,
                                }
                                .into(),
                            ),
                            _ => {}
                        },
                        _ => {}
                    },
                    _ => {}
                }
            }
        }
        _ => {}
    }
}
