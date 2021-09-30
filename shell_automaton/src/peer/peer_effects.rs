use redux_rs::{ActionWithId, Store};
use std::io::{self, Read, Write};
use tezos_messages::p2p::binary_message::CONTENT_LENGTH_FIELD_BYTES;

use crate::action::Action;
use crate::peer::binary_message::write::peer_binary_message_write_state::PeerBinaryMessageWriteState;
use crate::peer::chunk::read::peer_chunk_read_actions::{
    PeerChunkReadErrorAction, PeerChunkReadPartAction,
};
use crate::peer::chunk::write::peer_chunk_write_state::PeerChunkWriteState;
use crate::peer::chunk::write::{PeerChunkWriteErrorAction, PeerChunkWritePartAction};
use crate::peer::PeerStatus;
use crate::service::{MioService, Service};
use crate::State;

use super::binary_message::read::peer_binary_message_read_state::PeerBinaryMessageReadState;
use super::chunk::read::peer_chunk_read_state::PeerChunkReadState;
use super::disconnection::PeerDisconnectedAction;
use super::handshaking::PeerHandshakingStatus;
use super::{PeerTryReadAction, PeerTryWriteAction};

pub fn peer_effects<S>(store: &mut Store<State, S, Action>, action: &ActionWithId<Action>)
where
    S: Service,
{
    match &action.action {
        // Handle peer related mio event.
        Action::P2pPeerEvent(event) => {
            if event.is_closed() {
                return store.dispatch(
                    PeerDisconnectedAction {
                        address: event.address(),
                    }
                    .into(),
                );
            }

            if event.is_writable() {
                store.dispatch(
                    PeerTryWriteAction {
                        address: event.address(),
                    }
                    .into(),
                );
            }

            if event.is_readable() {
                store.dispatch(
                    PeerTryReadAction {
                        address: event.address(),
                    }
                    .into(),
                );
            }
        }
        Action::PeerTryWrite(action) => {
            let peer = match store.state.get().peers.get(&action.address) {
                Some(v) => v,
                None => return,
            };

            let peer_token = match &peer.status {
                PeerStatus::Handshaking(s) => s.token,
                _ => return,
            };

            let peer_stream = match store.service.mio().peer_get(peer_token) {
                Some(peer) => &mut peer.stream,
                None => return,
            };

            let chunk_state = match &peer.status {
                PeerStatus::Handshaking(handshaking) => match &handshaking.status {
                    PeerHandshakingStatus::ConnectionMessageWritePending {
                        chunk_state, ..
                    } => chunk_state,
                    PeerHandshakingStatus::MetadataMessageWritePending {
                        binary_message_state,
                        ..
                    }
                    | PeerHandshakingStatus::AckMessageWritePending {
                        binary_message_state,
                        ..
                    } => match binary_message_state {
                        PeerBinaryMessageWriteState::Pending { chunk, .. } => &chunk.state,
                        _ => return,
                    },
                    _ => return,
                },
                _ => return,
            };

            if let PeerChunkWriteState::Pending {
                chunk,
                written: prev_written,
            } = chunk_state
            {
                match peer_stream.write(&chunk.raw()[*prev_written..]) {
                    Ok(written) if written > 0 => {
                        eprintln!("<<< {}", hex::encode(&chunk.raw()[*prev_written..written]));
                        store.dispatch(
                            PeerChunkWritePartAction {
                                address: action.address,
                                written,
                            }
                            .into(),
                        )
                    }
                    Ok(_) => {}
                    Err(ref err) if err.kind() == io::ErrorKind::WouldBlock => {}
                    Err(err) => store.dispatch(
                        PeerChunkWriteErrorAction {
                            address: action.address,
                            error: err.into(),
                        }
                        .into(),
                    ),
                }
            }
        }
        Action::PeerTryRead(action) => {
            let peer = match store.state.get().peers.get(&action.address) {
                Some(v) => v,
                None => return,
            };

            let peer_token = match &peer.status {
                PeerStatus::Handshaking(s) => s.token,
                _ => return,
            };

            let peer_stream = match store.service.mio().peer_get(peer_token) {
                Some(peer) => &mut peer.stream,
                None => return,
            };

            let chunk_state = match &peer.status {
                PeerStatus::Handshaking(handshaking) => match &handshaking.status {
                    PeerHandshakingStatus::ConnectionMessageReadPending { chunk_state, .. } => {
                        chunk_state
                    }
                    PeerHandshakingStatus::MetadataMessageReadPending {
                        binary_message_state,
                        ..
                    }
                    | PeerHandshakingStatus::AckMessageReadPending {
                        binary_message_state,
                        ..
                    } => match binary_message_state {
                        PeerBinaryMessageReadState::PendingFirstChunk { chunk, .. }
                        | PeerBinaryMessageReadState::Pending { chunk, .. } => &chunk.state,
                        _ => return,
                    },
                    _ => return,
                },
                _ => return,
            };

            let bytes_to_read = match chunk_state {
                PeerChunkReadState::PendingSize { buffer } => {
                    CONTENT_LENGTH_FIELD_BYTES - buffer.len()
                }
                PeerChunkReadState::PendingBody { buffer, size } => size - buffer.len(),
                _ => return,
            };

            let mut buff = vec![0; bytes_to_read];
            match peer_stream.read(&mut buff) {
                Ok(bytes) if bytes > 0 => {
                    eprintln!(">>> {}", hex::encode(&buff));
                    store.dispatch(
                        PeerChunkReadPartAction {
                            address: action.address,
                            bytes: buff[..bytes].to_vec(),
                        }
                        .into(),
                    );
                }
                Ok(_) => todo!("handle eof"),
                Err(ref err) if err.kind() == io::ErrorKind::WouldBlock => (),
                Err(err) => store.dispatch(
                    PeerChunkReadErrorAction {
                        address: action.address,
                        error: err.into(),
                    }
                    .into(),
                ),
            }
        }
        _ => {}
    }
}
