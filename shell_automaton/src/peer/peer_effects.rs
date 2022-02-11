// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT
use std::io;
use tezos_messages::p2p::binary_message::CONTENT_LENGTH_FIELD_BYTES;

use crate::paused_loops::{PausedLoop, PausedLoopsAddAction};
use crate::request::RequestId;
use crate::service::storage_service::{StorageResponseError, StorageResponseSuccess};
use crate::service::{MioService, Service};
use crate::storage::request::StorageRequestor;
use crate::{Action, ActionWithMeta, Store};

use super::binary_message::read::PeerBinaryMessageReadState;
use super::binary_message::write::PeerBinaryMessageWriteState;
use super::chunk::read::PeerChunkReadState;
use super::chunk::read::{PeerChunkReadErrorAction, PeerChunkReadPartAction};
use super::chunk::write::{
    PeerChunkWriteErrorAction, PeerChunkWritePartAction, PeerChunkWriteState,
};
use super::connection::closed::PeerConnectionClosedAction;
use super::connection::incoming::{
    PeerConnectionIncomingState, PeerConnectionIncomingSuccessAction,
};
use super::connection::outgoing::{
    PeerConnectionOutgoingState, PeerConnectionOutgoingSuccessAction,
};
use super::connection::PeerConnectionState;
use super::disconnection::PeerDisconnectAction;
use super::handshaking::PeerHandshakingStatus;
use super::message::read::PeerMessageReadState;
use super::remote_requests::block_header_get::{
    PeerRemoteRequestsBlockHeaderGetErrorAction, PeerRemoteRequestsBlockHeaderGetSuccessAction,
};
use super::remote_requests::block_operations_get::{
    PeerRemoteRequestsBlockOperationsGetErrorAction,
    PeerRemoteRequestsBlockOperationsGetSuccessAction,
};
use super::{
    PeerHandshaked, PeerIOLoopResult, PeerStatus, PeerTryReadLoopFinishAction,
    PeerTryReadLoopStartAction, PeerTryWriteLoopFinishAction, PeerTryWriteLoopStartAction,
};

pub fn peer_effects<S>(store: &mut Store<S>, action: &ActionWithMeta)
where
    S: Service,
{
    match &action.action {
        Action::WakeupEvent(_) => {
            let quota_restore_duration_millis =
                store.state.get().config.quota.restore_duration_millis as u128;
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
                store.dispatch(PeerTryReadLoopStartAction { address });
            }
            for address in write_addresses {
                store.dispatch(PeerTryWriteLoopStartAction { address });
            }
        }
        // Handle peer related mio event.
        Action::P2pPeerEvent(event) => {
            let address = event.address();

            if event.is_closed() {
                return {
                    store.dispatch(PeerConnectionClosedAction { address });
                };
            }

            if event.is_writable() {
                // when we receive first writable event from mio,
                // that's when we know that we successfuly connected
                // to the peer.
                let peer = match store.state.get().peers.get(&address) {
                    Some(v) => v,
                    None => return,
                };
                match &peer.status {
                    PeerStatus::Connecting(connection_state) => match connection_state {
                        PeerConnectionState::Incoming(PeerConnectionIncomingState::Pending {
                            ..
                        }) => {
                            store.dispatch(PeerConnectionIncomingSuccessAction { address });
                        }
                        PeerConnectionState::Outgoing(PeerConnectionOutgoingState::Pending {
                            ..
                        }) => {
                            store.dispatch(PeerConnectionOutgoingSuccessAction { address });
                        }
                        _ => {}
                    },
                    _ => {}
                }

                store.dispatch(PeerTryWriteLoopStartAction { address });
            }

            if event.is_readable() {
                store.dispatch(PeerTryReadLoopStartAction { address });
            }
        }
        Action::PeerTryWriteLoopStart(action) => {
            let address = action.address;
            let peer_max_io_syscalls = store.state().config.peer_max_io_syscalls;

            let finish = |store: &mut Store<S>, address, result| {
                store.dispatch(PeerTryWriteLoopFinishAction { address, result });
            };

            for _ in 0..peer_max_io_syscalls {
                let peer = match store.state.get().peers.get(&address) {
                    Some(v) => v,
                    None => return,
                };

                if peer.quota.reject_write {
                    return finish(store, action.address, PeerIOLoopResult::ByteQuotaReached);
                }

                let peer_token = match &peer.status {
                    PeerStatus::Handshaking(s) => s.token,
                    PeerStatus::Handshaked(s) => s.token,
                    _ => return finish(store, action.address, PeerIOLoopResult::NotReady),
                };

                let mut mio_peer = match store.service.mio().peer_get(peer_token) {
                    Some(peer) => peer,
                    None => {
                        return {
                            store.dispatch(PeerDisconnectAction { address });
                        }
                    }
                };

                let chunk_state = match &peer.status {
                    PeerStatus::Handshaking(handshaking) => match &handshaking.status {
                        PeerHandshakingStatus::ConnectionMessageWritePending {
                            chunk_state,
                            ..
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
                            _ => return finish(store, address, PeerIOLoopResult::NotReady),
                        },
                        _ => return finish(store, address, PeerIOLoopResult::NotReady),
                    },
                    PeerStatus::Handshaked(PeerHandshaked { message_write, .. }) => {
                        match &message_write.current {
                            PeerBinaryMessageWriteState::Pending { chunk, .. } => &chunk.state,
                            _ => return finish(store, address, PeerIOLoopResult::NotReady),
                        }
                    }
                    _ => return finish(store, address, PeerIOLoopResult::NotReady),
                };

                let (chunk, prev_written) = match chunk_state {
                    PeerChunkWriteState::Pending { chunk, written } => (chunk, written),
                    _ => return finish(store, address, PeerIOLoopResult::NotReady),
                };

                match mio_peer.write(&chunk.raw()[*prev_written..]) {
                    Ok(written) if written > 0 => {
                        store.dispatch(PeerChunkWritePartAction {
                            address: action.address,
                            written,
                        });
                    }
                    Ok(_) => return finish(store, address, PeerIOLoopResult::FullyConsumed),
                    Err(ref err) if err.kind() == io::ErrorKind::WouldBlock => {
                        return finish(store, address, PeerIOLoopResult::FullyConsumed);
                    }
                    Err(err) => {
                        store.dispatch(PeerChunkWriteErrorAction {
                            address: action.address,
                            error: err.into(),
                        });
                        return;
                    }
                }
            }

            finish(store, address, PeerIOLoopResult::MaxIOSyscallBoundReached);
        }
        Action::PeerTryWriteLoopFinish(action) => match &action.result {
            PeerIOLoopResult::MaxIOSyscallBoundReached => {
                store.dispatch(PausedLoopsAddAction {
                    data: PausedLoop::PeerTryWrite {
                        peer_address: action.address,
                    },
                });
            }
            _ => {}
        },
        Action::PeerTryReadLoopStart(action) => {
            let address = action.address;
            let peer_max_io_syscalls = store.state().config.peer_max_io_syscalls;

            let finish = |store: &mut Store<S>, address, result| {
                store.dispatch(PeerTryReadLoopFinishAction { address, result });
            };

            for _ in 0..peer_max_io_syscalls {
                let peer = match store.state.get().peers.get(&address) {
                    Some(v) => v,
                    None => return,
                };

                if peer.quota.reject_read {
                    return finish(store, action.address, PeerIOLoopResult::ByteQuotaReached);
                }

                let peer_token = match &peer.status {
                    PeerStatus::Handshaking(s) => s.token,
                    PeerStatus::Handshaked(s) => s.token,
                    _ => return finish(store, action.address, PeerIOLoopResult::NotReady),
                };

                let mut mio_peer = match store.service.mio().peer_get(peer_token) {
                    Some(peer) => peer,
                    None => {
                        return {
                            store.dispatch(PeerDisconnectAction { address });
                        }
                    }
                };

                let chunk_state = match &peer.status {
                    PeerStatus::Handshaking(handshaking) => match &handshaking.status {
                        PeerHandshakingStatus::ConnectionMessageReadPending {
                            chunk_state, ..
                        } => chunk_state,
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
                            _ => return finish(store, address, PeerIOLoopResult::NotReady),
                        },
                        _ => return finish(store, address, PeerIOLoopResult::NotReady),
                    },
                    PeerStatus::Handshaked(handshaked) => match &handshaked.message_read {
                        PeerMessageReadState::Pending {
                            binary_message_read,
                        } => match binary_message_read {
                            PeerBinaryMessageReadState::PendingFirstChunk { chunk, .. }
                            | PeerBinaryMessageReadState::Pending { chunk, .. } => &chunk.state,
                            _ => return finish(store, address, PeerIOLoopResult::NotReady),
                        },
                        _ => return finish(store, address, PeerIOLoopResult::NotReady),
                    },
                    _ => return finish(store, address, PeerIOLoopResult::NotReady),
                };

                let bytes_to_read = match chunk_state {
                    PeerChunkReadState::PendingSize { buffer } => {
                        CONTENT_LENGTH_FIELD_BYTES - buffer.len()
                    }
                    PeerChunkReadState::PendingBody { buffer, size } => size - buffer.len(),
                    _ => return finish(store, address, PeerIOLoopResult::NotReady),
                };
                // debug_assert!(bytes_to_read > 0);

                match mio_peer.read(bytes_to_read) {
                    Ok(bytes) if bytes.len() > 0 => {
                        let bytes = bytes.to_vec();
                        store.dispatch(PeerChunkReadPartAction { address, bytes });
                    }
                    Ok(_) => return finish(store, address, PeerIOLoopResult::FullyConsumed),
                    Err(ref err) if err.kind() == io::ErrorKind::WouldBlock => {
                        return finish(store, address, PeerIOLoopResult::FullyConsumed);
                    }
                    Err(err) => {
                        return {
                            store.dispatch(PeerChunkReadErrorAction {
                                address,
                                error: err.into(),
                            });
                        }
                    }
                }
            }

            finish(store, address, PeerIOLoopResult::MaxIOSyscallBoundReached);
        }
        Action::PeerTryReadLoopFinish(action) => match &action.result {
            PeerIOLoopResult::MaxIOSyscallBoundReached => {
                store.dispatch(PausedLoopsAddAction {
                    data: PausedLoop::PeerTryRead {
                        peer_address: action.address,
                    },
                });
            }
            _ => {}
        },
        Action::StorageResponseReceived(content) => {
            let address = match &content.requestor {
                StorageRequestor::Peer(address) => *address,
                _ => return,
            };
            let state = store.state.get();
            let peer = match state.peers.get_handshaked(&address) {
                Some(v) => v,
                None => {
                    slog::debug!(&state.log, "Peer not found for storage response";
                        "address" => address.to_string(),
                        "response" => format!("{:?}", content.response));
                    return;
                }
            };

            let req_id = content.response.req_id;
            let req_id_matches = |expected_req_id: Option<RequestId>| {
                let result = expected_req_id
                    .and_then(|expected_id| req_id.map(|id| id == expected_id))
                    .unwrap_or(false);
                if result {
                    slog::debug!(&state.log, "Unexpected storage response for peer";
                        "address" => address.to_string(),
                        "response" => format!("{:?}", content.response));
                }
                result
            };

            match &content.response.result {
                Ok(StorageResponseSuccess::BlockHeaderGetSuccess(_, result)) => {
                    if !req_id_matches(
                        peer.remote_requests
                            .block_header_get
                            .current
                            .storage_req_id(),
                    ) {
                        return;
                    }
                    store.dispatch(PeerRemoteRequestsBlockHeaderGetSuccessAction {
                        address,
                        result: result.clone(),
                    });
                }
                Err(StorageResponseError::BlockHeaderGetError(_, error)) => {
                    if !req_id_matches(
                        peer.remote_requests
                            .block_header_get
                            .current
                            .storage_req_id(),
                    ) {
                        return;
                    }
                    store.dispatch(PeerRemoteRequestsBlockHeaderGetErrorAction {
                        address,
                        error: error.clone(),
                    });
                }
                Ok(StorageResponseSuccess::BlockOperationsGetSuccess(result)) => {
                    if !req_id_matches(
                        peer.remote_requests
                            .block_operations_get
                            .current
                            .storage_req_id(),
                    ) {
                        return;
                    }
                    store.dispatch(PeerRemoteRequestsBlockOperationsGetSuccessAction {
                        address,
                        result: result.clone(),
                    });
                }
                Err(StorageResponseError::BlockOperationsGetError(error)) => {
                    if !req_id_matches(
                        peer.remote_requests
                            .block_operations_get
                            .current
                            .storage_req_id(),
                    ) {
                        return;
                    }
                    store.dispatch(PeerRemoteRequestsBlockOperationsGetErrorAction {
                        address,
                        error: error.clone(),
                    });
                }
                _ => {}
            }
        }
        _ => {}
    }
}
