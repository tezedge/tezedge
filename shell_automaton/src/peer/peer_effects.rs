use redux_rs::{ActionWithId, Store};
use std::io::{self, Read, Write};
use tezos_messages::p2p::binary_message::CONTENT_LENGTH_FIELD_BYTES;

use crate::service::{MioService, Service};
use crate::{Action, State};

use super::binary_message::read::PeerBinaryMessageReadState;
use super::binary_message::write::PeerBinaryMessageWriteState;
use super::chunk::read::PeerChunkReadState;
use super::chunk::read::{PeerChunkReadErrorAction, PeerChunkReadPartAction};
use super::chunk::write::{
    PeerChunkWriteErrorAction, PeerChunkWritePartAction, PeerChunkWriteState,
};
use super::connection::closed::PeerConnectionClosedAction;
use super::handshaking::PeerHandshakingStatus;
use super::message::read::PeerMessageReadState;
use super::{PeerHandshaked, PeerStatus, PeerTryReadAction, PeerTryWriteAction};

pub fn peer_effects<S>(store: &mut Store<State, S, Action>, action: &ActionWithId<Action>)
where
    S: Service,
{
    match &action.action {
        Action::WakeupEvent(_) => {
            let quota_restore_duration_millis =
                store.state.get().config.quota.restore_duration_millis;
            let read_addresses = store
                .state
                .get()
                .peers
                .iter()
                .filter_map(|(address, peer)| {
                    if peer.quota.reject_read
                        && action
                            .id
                            .duration_since(peer.quota.read_timestamp)
                            .as_millis()
                            >= quota_restore_duration_millis
                    {
                        Some(address)
                    } else {
                        None
                    }
                })
                .cloned()
                .collect::<Vec<_>>();

            let write_addresses = store
                .state
                .get()
                .peers
                .iter()
                .filter_map(|(address, peer)| {
                    if peer.quota.reject_write
                        && action
                            .id
                            .duration_since(peer.quota.write_timestamp)
                            .as_millis()
                            >= quota_restore_duration_millis
                    {
                        Some(address)
                    } else {
                        None
                    }
                })
                .cloned()
                .collect::<Vec<_>>();

            for address in read_addresses {
                store.dispatch(PeerTryReadAction { address }.into());
            }
            for address in write_addresses {
                store.dispatch(PeerTryWriteAction { address }.into());
            }
        }
        // Handle peer related mio event.
        Action::P2pPeerEvent(event) => {
            if event.is_closed() {
                return store.dispatch(
                    PeerConnectionClosedAction {
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

            if peer.quota.reject_write {
                return;
            }

            let peer_token = match &peer.status {
                PeerStatus::Handshaking(s) => s.token,
                PeerStatus::Handshaked(s) => s.token,
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
                PeerStatus::Handshaked(PeerHandshaked { message_write, .. }) => {
                    match &message_write.current {
                        PeerBinaryMessageWriteState::Pending { chunk, .. } => &chunk.state,
                        _ => return,
                    }
                }
                _ => return,
            };

            if let PeerChunkWriteState::Pending {
                chunk,
                written: prev_written,
            } = chunk_state
            {
                match peer_stream.write(&chunk.raw()[*prev_written..]) {
                    Ok(written) if written > 0 => store.dispatch(
                        PeerChunkWritePartAction {
                            address: action.address,
                            written,
                        }
                        .into(),
                    ),
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

            if peer.quota.reject_read {
                return;
            }

            let peer_token = match &peer.status {
                PeerStatus::Handshaking(s) => s.token,
                PeerStatus::Handshaked(s) => s.token,
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
                PeerStatus::Handshaked(handshaked) => match &handshaked.message_read {
                    PeerMessageReadState::Pending {
                        binary_message_read,
                    } => match binary_message_read {
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
            debug_assert!(bytes_to_read > 0);

            let mut buff = vec![0; bytes_to_read];
            match peer_stream.read(&mut buff) {
                Ok(bytes) if bytes > 0 => {
                    store.dispatch(
                        PeerChunkReadPartAction {
                            address: action.address,
                            bytes: buff[..bytes].to_vec(),
                        }
                        .into(),
                    );
                }
                Ok(_) => {}
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
