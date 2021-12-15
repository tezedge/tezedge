// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::net::SocketAddr;

use tezos_encoding::binary_writer::BinaryWriterError;
use tezos_messages::p2p::binary_message::BinaryWrite;

use crate::peer::binary_message::write::{
    PeerBinaryMessageWriteSetContentAction, PeerBinaryMessageWriteState,
};
use crate::peer::message::write::{PeerMessageWriteErrorAction, PeerMessageWriteSuccessAction};
use crate::peers::graylist::PeersGraylistAddressAction;
use crate::service::Service;
use crate::{Action, ActionWithMeta, Store};

use super::PeerMessageWriteNextAction;

fn binary_message_write_init<S: Service>(
    store: &mut Store<S>,
    address: SocketAddr,
    encoded_message: Result<Vec<u8>, BinaryWriterError>,
) -> bool {
    match encoded_message {
        Ok(bytes) => store.dispatch(PeerBinaryMessageWriteSetContentAction {
            address,
            message: bytes,
        }),
        Err(err) => store.dispatch(PeerMessageWriteErrorAction {
            address,
            error: err.into(),
        }),
    }
}

pub fn peer_message_write_effects<S>(store: &mut Store<S>, action: &ActionWithMeta)
where
    S: Service,
{
    match &action.action {
        Action::PeerMessageWriteNext(action) => {
            let peer = match store.state.get().peers.get(&action.address) {
                Some(peer) => match peer.status.as_handshaked() {
                    Some(v) => v,
                    None => return,
                },
                None => return,
            };

            if let PeerBinaryMessageWriteState::Init { .. } = &peer.message_write.current {
                if let Some(next_message) = peer.message_write.queue.front() {
                    let message_encode_result = next_message.as_bytes();
                    binary_message_write_init(store, action.address, message_encode_result);
                }
            }
        }
        Action::PeerMessageWriteInit(action) => {
            let peer = match store.state.get().peers.get(&action.address) {
                Some(peer) => match peer.status.as_handshaked() {
                    Some(v) => v,
                    None => return,
                },
                None => return,
            };

            if let PeerBinaryMessageWriteState::Init { .. } = &peer.message_write.current {
                binary_message_write_init(store, action.address, action.message.as_bytes());
            }
        }
        Action::PeerBinaryMessageWriteReady(action) => {
            let peer = match store.state().peers.get(&action.address) {
                Some(peer) => match peer.status.as_handshaked() {
                    Some(handshaked) => handshaked,
                    None => return,
                },
                None => return,
            };

            if let PeerBinaryMessageWriteState::Ready { .. } = &peer.message_write.current {
                store.dispatch(PeerMessageWriteSuccessAction {
                    address: action.address,
                });
            }
        }
        Action::PeerMessageWriteSuccess(action) => {
            store.dispatch(PeerMessageWriteNextAction {
                address: action.address,
            });
        }
        Action::PeerMessageWriteError(action) => {
            store.dispatch(PeersGraylistAddressAction {
                address: action.address,
            });
        }
        _ => {}
    }
}
