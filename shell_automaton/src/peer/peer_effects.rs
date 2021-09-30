use bytes::Buf;
use redux_rs::{ActionWithId, Store};
use std::io::{Read, Write};
use tezos_messages::p2p::binary_message::CONTENT_LENGTH_FIELD_BYTES;

use crate::action::Action;
use crate::peer::PeerStatus;
use crate::service::{MioService, Service};
use crate::State;

use super::disconnection::PeerDisconnectedAction;
use super::handshaking::connection_message::read::{
    PeerConnectionMessagePartReadAction, PeerConnectionMessageReadErrorAction,
};
use super::handshaking::connection_message::write::{
    PeerConnectionMessagePartWrittenAction, PeerConnectionMessageWriteErrorAction,
};
use super::handshaking::{MessageReadState, MessageWriteState, PeerHandshakingStatus};
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

            match &peer.status {
                PeerStatus::Handshaking(handshaking) => match &handshaking.status {
                    PeerHandshakingStatus::ConnectionMessageWrite { conn_msg, status } => {
                        let bytes = match status {
                            MessageWriteState::Idle => conn_msg.raw(),
                            MessageWriteState::Pending { written } => &conn_msg.raw()[*written..],
                            _ => return,
                        };
                        match peer_stream.write(bytes) {
                            Ok(written) => {
                                if written == 0 {
                                    return;
                                }
                                store.dispatch(
                                    PeerConnectionMessagePartWrittenAction {
                                        address: action.address,
                                        bytes_written: written,
                                    }
                                    .into(),
                                );
                            }
                            Err(err) => {
                                match err.kind() {
                                    std::io::ErrorKind::WouldBlock => return,
                                    _ => {}
                                }
                                store.dispatch(
                                    PeerConnectionMessageWriteErrorAction {
                                        address: action.address,
                                        error: err.kind().into(),
                                    }
                                    .into(),
                                );
                            }
                        }
                    }
                    _ => return,
                },
                _ => return,
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

            match &peer.status {
                PeerStatus::Handshaking(handshaking) => {
                    // set dispatch_error and dispatch_success methods
                    // based on which message we are expecting to read.
                    let (bytes_to_read, dispatch_success, dispatch_error) =
                        match &handshaking.status {
                            PeerHandshakingStatus::ConnectionMessageRead { status, .. } => {
                                let bytes_to_read = match status {
                                    MessageReadState::Idle => CONTENT_LENGTH_FIELD_BYTES,
                                    MessageReadState::Pending { buffer } => {
                                        buffer.bytes_left().unwrap_or(CONTENT_LENGTH_FIELD_BYTES)
                                    }
                                    _ => return,
                                };
                                (
                                    bytes_to_read,
                                    |store: &mut Store<State, S, Action>, bytes| {
                                        store.dispatch(
                                            PeerConnectionMessagePartReadAction {
                                                address: action.address,
                                                bytes,
                                            }
                                            .into(),
                                        );
                                    },
                                    |store: &mut Store<State, S, Action>, error| {
                                        store.dispatch(
                                            PeerConnectionMessageReadErrorAction {
                                                address: action.address,
                                                error,
                                            }
                                            .into(),
                                        );
                                    },
                                )
                            }
                            _ => return,
                        };

                    &bytes_to_read;
                    let mut buf = vec![0; bytes_to_read];
                    match peer_stream.read(&mut buf) {
                        Ok(read_bytes) => {
                            dispatch_success(store, buf[..read_bytes].to_vec());
                        }
                        Err(err) => match err.kind() {
                            std::io::ErrorKind::WouldBlock => return,
                            err => dispatch_error(store, err.into()),
                        },
                    }
                }
                _ => return,
            }
        }
        _ => {}
    }
}
