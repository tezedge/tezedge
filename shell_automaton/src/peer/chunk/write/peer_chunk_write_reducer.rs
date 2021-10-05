use redux_rs::ActionWithId;

use crate::peer::binary_message::write::PeerBinaryMessageWriteState;
use crate::peer::chunk::write::PeerChunkWriteState;
use crate::peer::handshaking::{PeerHandshaking, PeerHandshakingStatus};
use crate::peer::PeerStatus;
use crate::{Action, State};

pub fn peer_chunk_write_reducer(state: &mut State, action: &ActionWithId<Action>) {
    match &action.action {
        Action::PeerChunkWriteSetContent(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                let chunk_state = match &mut peer.status {
                    PeerStatus::Handshaking(PeerHandshaking { status, .. }) => match status {
                        PeerHandshakingStatus::ConnectionMessageWritePending {
                            chunk_state,
                            ..
                        } => chunk_state,
                        PeerHandshakingStatus::MetadataMessageWritePending {
                            binary_message_state: PeerBinaryMessageWriteState::Pending { chunk, .. },
                            ..
                        }
                        | PeerHandshakingStatus::AckMessageWritePending {
                            binary_message_state: PeerBinaryMessageWriteState::Pending { chunk, .. },
                            ..
                        } => &mut chunk.state,
                        _ => return,
                    },
                    _ => return,
                };

                if let PeerChunkWriteState::Init = chunk_state {
                    *chunk_state = PeerChunkWriteState::UnencryptedContent {
                        content: action.content.clone(),
                    };
                }
            }
        }
        Action::PeerChunkWriteEncryptContent(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                match &mut peer.status {
                    PeerStatus::Handshaking(PeerHandshaking { status, .. }) => match status {
                        PeerHandshakingStatus::MetadataMessageWritePending {
                            binary_message_state: PeerBinaryMessageWriteState::Pending { chunk, .. },
                            ..
                        }
                        | PeerHandshakingStatus::AckMessageWritePending {
                            binary_message_state: PeerBinaryMessageWriteState::Pending { chunk, .. },
                            ..
                        } => match chunk.state {
                            PeerChunkWriteState::UnencryptedContent { .. } => {
                                chunk.state = PeerChunkWriteState::EncryptedContent {
                                    content: action.encrypted_content.clone(),
                                };
                                chunk.crypto.increment_nonce();
                            }
                            _ => {}
                        },
                        _ => {}
                    },
                    _ => {}
                }
            }
        }
        Action::PeerChunkWriteCreateChunk(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                let chunk_state = match &mut peer.status {
                    PeerStatus::Handshaking(PeerHandshaking { status, .. }) => match status {
                        PeerHandshakingStatus::ConnectionMessageWritePending {
                            chunk_state,
                            ..
                        } => match chunk_state {
                            PeerChunkWriteState::UnencryptedContent { .. } => chunk_state,
                            _ => return,
                        },
                        PeerHandshakingStatus::MetadataMessageWritePending {
                            binary_message_state: PeerBinaryMessageWriteState::Pending { chunk, .. },
                            ..
                        }
                        | PeerHandshakingStatus::AckMessageWritePending {
                            binary_message_state: PeerBinaryMessageWriteState::Pending { chunk, .. },
                            ..
                        } => match chunk.state {
                            PeerChunkWriteState::EncryptedContent { .. } => &mut chunk.state,
                            _ => return,
                        },
                        _ => return,
                    },
                    _ => return,
                };

                *chunk_state = PeerChunkWriteState::Pending {
                    chunk: action.chunk.clone(),
                    written: 0,
                };
            }
        }
        Action::PeerChunkWritePart(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                match &mut peer.status {
                    PeerStatus::Handshaking(PeerHandshaking { status, .. }) => {
                        let chunk_state = match status {
                            PeerHandshakingStatus::ConnectionMessageWritePending {
                                chunk_state,
                                ..
                            } => chunk_state,
                            PeerHandshakingStatus::MetadataMessageWritePending {
                                binary_message_state:
                                    PeerBinaryMessageWriteState::Pending { chunk, .. },
                                ..
                            }
                            | PeerHandshakingStatus::AckMessageWritePending {
                                binary_message_state:
                                    PeerBinaryMessageWriteState::Pending { chunk, .. },
                                ..
                            } => &mut chunk.state,
                            _ => return,
                        };

                        if let PeerChunkWriteState::Pending { chunk, written } = chunk_state {
                            if *written + action.written < chunk.raw().len() {
                                *written += action.written;
                            } else {
                                *chunk_state = PeerChunkWriteState::Ready;
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
