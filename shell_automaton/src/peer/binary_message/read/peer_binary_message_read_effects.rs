use redux_rs::{ActionWithId, Store};
use tezos_messages::p2p::{
    binary_message::SizeFromChunk,
    encoding::{ack::AckMessage, metadata::MetadataMessage},
};

use crate::{
    action::Action,
    peer::{
        chunk::read::{
            peer_chunk_read_actions::PeerChunkReadInitAction,
            peer_chunk_read_state::{PeerChunkRead, PeerChunkReadState},
        },
        handshaking::{PeerHandshaking, PeerHandshakingStatus},
        PeerStatus,
    },
    service::Service,
    State,
};

use super::{
    peer_binary_message_read_actions::{
        PeerBinaryMessageReadErrorAction, PeerBinaryMessageReadReadyAction,
        PeerBinaryMessageReadSizeReadyAction,
    },
    peer_binary_message_read_state::PeerBinaryMessageReadState,
};

pub fn peer_binary_message_read_effects<S>(
    store: &mut Store<State, S, Action>,
    action: &ActionWithId<Action>,
) where
    S: Service,
{
    match &action.action {
        Action::PeerBinaryMessageReadInit(action) => {
            if let Some(peer) = store.state.get().peers.get(&action.address) {
                match &peer.status {
                    PeerStatus::Handshaking(PeerHandshaking { status, .. }) => match status {
                        PeerHandshakingStatus::MetadataMessageReadPending {
                            binary_message_state,
                            ..
                        }
                        | PeerHandshakingStatus::AckMessageReadPending {
                            binary_message_state,
                            ..
                        } => match binary_message_state {
                            PeerBinaryMessageReadState::PendingFirstChunk { .. } => store.dispatch(
                                PeerChunkReadInitAction {
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
        Action::PeerChunkReadReady(action) => {
            if let Some(peer) = store.state.get().peers.get(&action.address) {
                match &peer.status {
                    PeerStatus::Handshaking(PeerHandshaking { status, .. }) => match status {
                        PeerHandshakingStatus::MetadataMessageReadPending {
                            binary_message_state:
                                PeerBinaryMessageReadState::PendingFirstChunk {
                                    chunk:
                                        PeerChunkRead {
                                            state: PeerChunkReadState::Ready { chunk },
                                            ..
                                        },
                                    ..
                                },
                            ..
                        } => match MetadataMessage::size_from_chunk(&chunk) {
                            Ok(size) => store.dispatch(
                                PeerBinaryMessageReadSizeReadyAction {
                                    address: action.address,
                                    size,
                                }
                                .into(),
                            ),
                            Err(err) => store.dispatch(
                                PeerBinaryMessageReadErrorAction {
                                    address: action.address,
                                    error: err.into(),
                                }
                                .into(),
                            ),
                        },
                        PeerHandshakingStatus::AckMessageReadPending {
                            binary_message_state:
                                PeerBinaryMessageReadState::PendingFirstChunk {
                                    chunk:
                                        PeerChunkRead {
                                            state: PeerChunkReadState::Ready { chunk },
                                            ..
                                        },
                                    ..
                                },
                            ..
                        } => match AckMessage::size_from_chunk(&chunk) {
                            Ok(size) => store.dispatch(
                                PeerBinaryMessageReadSizeReadyAction {
                                    address: action.address,
                                    size,
                                }
                                .into(),
                            ),
                            Err(err) => store.dispatch(
                                PeerBinaryMessageReadErrorAction {
                                    address: action.address,
                                    error: err.into(),
                                }
                                .into(),
                            ),
                        },
                        PeerHandshakingStatus::MetadataMessageReadPending {
                            binary_message_state,
                            ..
                        }
                        | PeerHandshakingStatus::AckMessageReadPending {
                            binary_message_state,
                            ..
                        } => match binary_message_state {
                            PeerBinaryMessageReadState::Pending { .. } => store.dispatch(
                                PeerChunkReadInitAction {
                                    address: action.address,
                                }
                                .into(),
                            ),
                            PeerBinaryMessageReadState::Ready { message, .. } => {
                                let message = message.clone();
                                store.dispatch(
                                    PeerBinaryMessageReadReadyAction {
                                        address: action.address,
                                        message,
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
        Action::PeerBinaryMessageReadSizeReady(action) => {
            if let Some(peer) = store.state.get().peers.get(&action.address) {
                match &peer.status {
                    PeerStatus::Handshaking(PeerHandshaking { status, .. }) => match status {
                        PeerHandshakingStatus::MetadataMessageReadPending {
                            binary_message_state,
                            ..
                        }
                        | PeerHandshakingStatus::AckMessageReadPending {
                            binary_message_state,
                            ..
                        } => match binary_message_state {
                            PeerBinaryMessageReadState::Pending { .. } => store.dispatch(
                                PeerChunkReadInitAction {
                                    address: action.address,
                                }
                                .into(),
                            ),
                            PeerBinaryMessageReadState::Ready { message, .. } => {
                                let message = message.clone();
                                store.dispatch(
                                    PeerBinaryMessageReadReadyAction {
                                        address: action.address,
                                        message,
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

        _ => {}
    }
}
