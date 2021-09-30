use redux_rs::ActionWithId;

use crate::{
    action::Action,
    peer::{
        chunk::read::peer_chunk_read_state::{PeerChunkRead, PeerChunkReadState},
        handshaking::{PeerHandshaking, PeerHandshakingStatus},
        PeerStatus,
    },
    State,
};

use super::peer_binary_message_read_state::PeerBinaryMessageReadState;

pub fn peer_binary_message_read_reducer(state: &mut State, action: &ActionWithId<Action>) {
    match &action.action {
        Action::PeerBinaryMessageReadInit(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                match &mut peer.status {
                    PeerStatus::Handshaking(PeerHandshaking { status, .. }) => match status {
                        PeerHandshakingStatus::MetadataMessageReadPending {
                            binary_message_state,
                            ..
                        }
                        | PeerHandshakingStatus::AckMessageReadPending {
                            binary_message_state,
                            ..
                        } => match binary_message_state {
                            PeerBinaryMessageReadState::Init { crypto } => {
                                *binary_message_state =
                                    PeerBinaryMessageReadState::PendingFirstChunk {
                                        chunk: PeerChunkRead {
                                            crypto: crypto.clone(),
                                            state: PeerChunkReadState::Init,
                                        },
                                    }
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
            if let Some(peer) = state.peers.get_mut(&action.address) {
                match &mut peer.status {
                    PeerStatus::Handshaking(PeerHandshaking { status, .. }) => match status {
                        PeerHandshakingStatus::MetadataMessageReadPending {
                            binary_message_state,
                            ..
                        }
                        | PeerHandshakingStatus::AckMessageReadPending {
                            binary_message_state,
                            ..
                        } => match binary_message_state {
                            PeerBinaryMessageReadState::PendingFirstChunk {
                                chunk:
                                    PeerChunkRead {
                                        crypto,
                                        state: PeerChunkReadState::Ready { chunk },
                                    },
                            } => {
                                if action.size < chunk.len() {
                                    *binary_message_state = PeerBinaryMessageReadState::Pending {
                                        buffer: Vec::new(),
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
                        },
                        _ => {}
                    },
                    _ => {}
                }
            }
        }
        Action::PeerBinaryMessageReadChunkReady(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                match &mut peer.status {
                    PeerStatus::Handshaking(PeerHandshaking { status, .. }) => match status {
                        PeerHandshakingStatus::MetadataMessageReadPending {
                            binary_message_state,
                            ..
                        }
                        | PeerHandshakingStatus::AckMessageReadPending {
                            binary_message_state,
                            ..
                        } => match binary_message_state {
                            PeerBinaryMessageReadState::Pending {
                                buffer,
                                size,
                                chunk:
                                    PeerChunkRead {
                                        crypto,
                                        state: PeerChunkReadState::Ready { chunk },
                                    },
                            } => {
                                if buffer.len() + chunk.len() <= *size {
                                    buffer.extend_from_slice(&chunk);
                                    if buffer.len() == *size {
                                        *binary_message_state = PeerBinaryMessageReadState::Ready {
                                            crypto: crypto.clone(),
                                            message: buffer.clone(),
                                        }
                                    }
                                }
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
