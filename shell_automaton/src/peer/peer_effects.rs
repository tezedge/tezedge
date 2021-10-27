// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use redux_rs::{ActionWithId, Store};
use std::io::{self, Read, Write};
use tezos_messages::p2p::binary_message::CONTENT_LENGTH_FIELD_BYTES;

use crate::paused_loops::{PausedLoop, PausedLoopsAddAction};
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
use super::{
    PeerHandshaked, PeerIOLoopResult, PeerStatus, PeerTryReadLoopFinishAction,
    PeerTryReadLoopStartAction, PeerTryWriteLoopFinishAction, PeerTryWriteLoopStartAction,
};

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
                store.dispatch(PeerTryReadLoopStartAction { address }.into());
            }
            for address in write_addresses {
                store.dispatch(PeerTryWriteLoopStartAction { address }.into());
            }
        }
        // Handle peer related mio event.
        Action::P2pPeerEvent(event) => {
            let address = event.address();

            if event.is_closed() {
                return store.dispatch(PeerConnectionClosedAction { address }.into());
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
                            store.dispatch(PeerConnectionIncomingSuccessAction { address }.into());
                        }
                        PeerConnectionState::Outgoing(PeerConnectionOutgoingState::Pending {
                            ..
                        }) => {
                            store.dispatch(PeerConnectionOutgoingSuccessAction { address }.into());
                        }
                        _ => {}
                    },
                    _ => {}
                }

                store.dispatch(PeerTryWriteLoopStartAction { address }.into());
            }

            if event.is_readable() {
                store.dispatch(PeerTryReadLoopStartAction { address }.into());
            }
        }
        Action::PeerTryWriteLoopStart(action) => {
            let address = action.address;
            let peer_max_io_syscalls = store.state().config.peer_max_io_syscalls;

            let finish = |store: &mut Store<State, S, Action>, address, result| {
                store.dispatch(PeerTryWriteLoopFinishAction { address, result }.into())
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

                let peer_stream = match store.service.mio().peer_get(peer_token) {
                    Some(peer) => &mut peer.stream,
                    None => return store.dispatch(PeerDisconnectAction { address }.into()),
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

                match peer_stream.write(&chunk.raw()[*prev_written..]) {
                    Ok(written) if written > 0 => store.dispatch(
                        PeerChunkWritePartAction {
                            address: action.address,
                            written,
                        }
                        .into(),
                    ),
                    Ok(_) => return finish(store, address, PeerIOLoopResult::FullyConsumed),
                    Err(ref err) if err.kind() == io::ErrorKind::WouldBlock => {
                        return finish(store, address, PeerIOLoopResult::FullyConsumed);
                    }
                    Err(err) => {
                        return store.dispatch(
                            PeerChunkWriteErrorAction {
                                address: action.address,
                                error: err.into(),
                            }
                            .into(),
                        )
                    }
                }
            }

            finish(store, address, PeerIOLoopResult::MaxIOSyscallBoundReached);
        }
        Action::PeerTryWriteLoopFinish(action) => match &action.result {
            PeerIOLoopResult::MaxIOSyscallBoundReached => store.dispatch(
                PausedLoopsAddAction {
                    data: PausedLoop::PeerTryWrite {
                        peer_address: action.address,
                    },
                }
                .into(),
            ),
            _ => {}
        },
        Action::PeerTryReadLoopStart(action) => {
            let address = action.address;
            let peer_max_io_syscalls = store.state().config.peer_max_io_syscalls;

            let finish = |store: &mut Store<State, S, Action>, address, result| {
                store.dispatch(PeerTryReadLoopFinishAction { address, result }.into())
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

                let peer_stream = match store.service.mio().peer_get(peer_token) {
                    Some(peer) => &mut peer.stream,
                    None => return store.dispatch(PeerDisconnectAction { address }.into()),
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
                debug_assert!(bytes_to_read > 0);

                let mut buff = vec![0; bytes_to_read];
                match peer_stream.read(&mut buff) {
                    Ok(bytes) if bytes > 0 => {
                        store.dispatch(
                            PeerChunkReadPartAction {
                                address,
                                bytes: buff[..bytes].to_vec(),
                            }
                            .into(),
                        );
                    }
                    Ok(_) => return finish(store, address, PeerIOLoopResult::FullyConsumed),
                    Err(ref err) if err.kind() == io::ErrorKind::WouldBlock => {
                        return finish(store, address, PeerIOLoopResult::FullyConsumed);
                    }
                    Err(err) => {
                        return store.dispatch(
                            PeerChunkReadErrorAction {
                                address,
                                error: err.into(),
                            }
                            .into(),
                        )
                    }
                }
            }

            finish(store, address, PeerIOLoopResult::MaxIOSyscallBoundReached);
        }
        Action::PeerTryReadLoopFinish(action) => match &action.result {
            PeerIOLoopResult::MaxIOSyscallBoundReached => store.dispatch(
                PausedLoopsAddAction {
                    data: PausedLoop::PeerTryRead {
                        peer_address: action.address,
                    },
                }
                .into(),
            ),
            _ => {}
        },
        _ => {}
    }
}
