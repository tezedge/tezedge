use redux_rs::{ActionWithId, Store};

use crate::{
    action::Action,
    peer::{
        chunk::read::{
            peer_chunk_read_actions::{
                PeerChunkReadDecryptAction, PeerChunkReadErrorAction, PeerChunkReadReadyAction,
            },
            peer_chunk_read_state::{PeerChunkReadError, PeerChunkReadState},
        },
        PeerStatus, PeerTryReadAction,
    },
    service::Service,
    State,
};

pub fn peer_chunk_read_effects<S>(
    store: &mut Store<State, S, Action>,
    action: &ActionWithId<Action>,
) where
    S: Service,
{
    match &action.action {
        Action::PeerChunkReadInit(action) => {
            store.dispatch(
                PeerTryReadAction {
                    address: action.address,
                }
                .into(),
            );
        }
        Action::PeerChunkReadPart(action) => {
            if let Some(peer) = store.state.get().peers.get(&action.address) {
                match &peer.status {
                    PeerStatus::Handshaking(handshaking) => match &handshaking.status {
                        crate::peer::handshaking::PeerHandshakingStatus::ConnectionMessageReadPending { chunk_state, .. } => match chunk_state {
                            PeerChunkReadState::PendingSize { .. } | PeerChunkReadState::PendingBody { .. } => {
                                store.dispatch(PeerTryReadAction { address: action.address }.into());
                            }
                            PeerChunkReadState::Ready { .. } => {
                                store.dispatch(PeerChunkReadReadyAction { address: action.address }.into());
                            }
                            _ => {}
                        },
                        crate::peer::handshaking::PeerHandshakingStatus::MetadataMessageReadPending { binary_message_state, .. } |
                        crate::peer::handshaking::PeerHandshakingStatus::AckMessageReadPending { binary_message_state, .. } => match binary_message_state {
                            crate::peer::binary_message::read::peer_binary_message_read_state::PeerBinaryMessageReadState::PendingFirstChunk { chunk } |
                            crate::peer::binary_message::read::peer_binary_message_read_state::PeerBinaryMessageReadState::Pending { chunk, .. } => match &chunk.state {
                                PeerChunkReadState::PendingSize { .. } | PeerChunkReadState::PendingBody { .. } => {
                                    store.dispatch(PeerTryReadAction { address: action.address }.into());
                                }
                                PeerChunkReadState::EncryptedReady { chunk_encrypted: chunk_content_encrypted } =>
                                    match chunk.crypto.decrypt(&chunk_content_encrypted) {
                                        Ok(decrypted_bytes) => store.dispatch(PeerChunkReadDecryptAction { address: action.address, decrypted_bytes }.into()),
                                        Err(err) => store.dispatch(PeerChunkReadErrorAction { address: action.address, error: PeerChunkReadError::from(err) }.into()),
                                    }
                                _ => {}
                            }
                            _ => {}
                        },
                        _ => return,
                    }
                    _ => return,
                };
            }
        }
        Action::PeerChunkReadDecrypt(action) => {
            if let Some(peer) = store.state.get().peers.get(&action.address) {
                match &peer.status {
                    PeerStatus::Handshaking(handshaking) => match &handshaking.status {
                        crate::peer::handshaking::PeerHandshakingStatus::MetadataMessageReadPending { binary_message_state, .. } |
                        crate::peer::handshaking::PeerHandshakingStatus::AckMessageReadPending { binary_message_state, .. } => match binary_message_state {
                            crate::peer::binary_message::read::peer_binary_message_read_state::PeerBinaryMessageReadState::PendingFirstChunk { chunk } |
                            crate::peer::binary_message::read::peer_binary_message_read_state::PeerBinaryMessageReadState::Pending { chunk, .. } => match &chunk.state {
                                PeerChunkReadState::Ready { .. } => {
                                    store.dispatch(PeerChunkReadReadyAction { address: action.address }.into());
                                }
                                _ => {}
                            }
                            _ => {}
                        },
                        _ => return,
                    }
                    _ => return,
                };
            }
        }
        _ => {}
    }
}
