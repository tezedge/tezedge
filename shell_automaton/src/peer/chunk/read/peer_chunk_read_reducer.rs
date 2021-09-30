use redux_rs::ActionWithId;
use tezos_messages::p2p::binary_message::CONTENT_LENGTH_FIELD_BYTES;

use crate::{
    action::Action,
    peer::{chunk::read::peer_chunk_read_state::PeerChunkReadState, PeerStatus},
    State,
};

pub fn peer_chunk_read_reducer(state: &mut State, action: &ActionWithId<Action>) {
    match &action.action {
        Action::PeerChunkReadInit(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                let chunk_state = match &mut peer.status {
                    PeerStatus::Handshaking(handshaking) => match &mut handshaking.status {
                        crate::peer::handshaking::PeerHandshakingStatus::ConnectionMessageReadPending { chunk_state, .. } => chunk_state,
                        crate::peer::handshaking::PeerHandshakingStatus::MetadataMessageReadPending { binary_message_state, .. } |
                        crate::peer::handshaking::PeerHandshakingStatus::AckMessageReadPending { binary_message_state, .. } => match binary_message_state {
                            crate::peer::binary_message::read::peer_binary_message_read_state::PeerBinaryMessageReadState::PendingFirstChunk { chunk } |
                            crate::peer::binary_message::read::peer_binary_message_read_state::PeerBinaryMessageReadState::Pending { chunk, .. } => &mut chunk.state,
                            _ => return,
                        },
                        _ => return,
                    }
                    _ => return,
                };

                if let PeerChunkReadState::Init = chunk_state {
                    *chunk_state = PeerChunkReadState::PendingSize { buffer: Vec::new() };
                }
            }
        }
        Action::PeerChunkReadPart(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                match &mut peer.status {
                    PeerStatus::Handshaking(handshaking) => match &mut handshaking.status {
                        crate::peer::handshaking::PeerHandshakingStatus::ConnectionMessageReadPending { chunk_state, .. } => match chunk_state {
                            PeerChunkReadState::PendingSize { buffer } => {
                                if buffer.len() + action.bytes.len() <= CONTENT_LENGTH_FIELD_BYTES {
                                    buffer.extend_from_slice(&action.bytes);
                                    if buffer.len() == CONTENT_LENGTH_FIELD_BYTES {
                                        let size = ((u16::from(buffer[0]) << 8) + u16::from(buffer[1])).into();
                                        *chunk_state = PeerChunkReadState::PendingBody { buffer: Vec::new(), size };
                                    }
                                }
                            }
                            PeerChunkReadState::PendingBody { buffer, size } => {
                                if buffer.len() + action.bytes.len() <= *size {
                                    buffer.extend_from_slice(&action.bytes);
                                    if buffer.len() == *size {
                                        *chunk_state = PeerChunkReadState::Ready { chunk: buffer.clone() };
                                    }
                                }
                            }
                            _ => {}
                        }
                        crate::peer::handshaking::PeerHandshakingStatus::MetadataMessageReadPending { binary_message_state, .. } |
                        crate::peer::handshaking::PeerHandshakingStatus::AckMessageReadPending { binary_message_state, .. } => match binary_message_state {
                            crate::peer::binary_message::read::peer_binary_message_read_state::PeerBinaryMessageReadState::PendingFirstChunk { chunk } |
                            crate::peer::binary_message::read::peer_binary_message_read_state::PeerBinaryMessageReadState::Pending { chunk, .. } => match &mut chunk.state {
                                PeerChunkReadState::PendingSize { buffer } => {
                                    if buffer.len() + action.bytes.len() <= CONTENT_LENGTH_FIELD_BYTES {
                                        buffer.extend_from_slice(&action.bytes);
                                        if buffer.len() == CONTENT_LENGTH_FIELD_BYTES {
                                            let size = ((u16::from(buffer[0]) << 8) + u16::from(buffer[1])).into();
                                            chunk.state = PeerChunkReadState::PendingBody { buffer: Vec::new(), size };
                                        }
                                    }
                                }
                                PeerChunkReadState::PendingBody { buffer, size } => {
                                    if buffer.len() + action.bytes.len() <= *size {
                                        buffer.extend_from_slice(&action.bytes);
                                        if buffer.len() == *size {
                                            chunk.state = PeerChunkReadState::EncryptedReady { chunk_encrypted: buffer.clone() };
                                        }
                                    }
                                }
                                _ => {}
                            }
                            _ => {}
                        },
                        _ => return,
                    }
                    _ => return,
                }
            }
        }
        Action::PeerChunkReadDecrypt(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                match &mut peer.status {
                    PeerStatus::Handshaking(handshaking) => match &mut handshaking.status {
                        crate::peer::handshaking::PeerHandshakingStatus::MetadataMessageReadPending { binary_message_state, .. } |
                        crate::peer::handshaking::PeerHandshakingStatus::AckMessageReadPending { binary_message_state, .. } => match binary_message_state {
                            crate::peer::binary_message::read::peer_binary_message_read_state::PeerBinaryMessageReadState::PendingFirstChunk { chunk } |
                            crate::peer::binary_message::read::peer_binary_message_read_state::PeerBinaryMessageReadState::Pending { chunk, .. } => {
                                if let PeerChunkReadState::EncryptedReady { .. } = &chunk.state {
                                    chunk.state = PeerChunkReadState::Ready { chunk: action.decrypted_bytes.clone() };
                                    chunk.crypto.increment_nonce();
                                }
                            },
                            _ => {},
                        },
                        _ => {},
                    }
                    _ => {},
                }
            }
        }
        _ => {}
    }
}
