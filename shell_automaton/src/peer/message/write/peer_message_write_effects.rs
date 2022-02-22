// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::net::SocketAddr;

use tezos_encoding::binary_writer::BinaryWriterError;
use tezos_messages::p2p::binary_message::BinaryWrite;
use tezos_messages::p2p::encoding::peer::PeerMessage;

use crate::peer::binary_message::write::{
    PeerBinaryMessageWriteSetContentAction, PeerBinaryMessageWriteState,
};
use crate::peer::message::write::{PeerMessageWriteErrorAction, PeerMessageWriteSuccessAction};
use crate::peers::graylist::PeersGraylistAddressAction;
use crate::service::{Service, StatisticsService};
use crate::{Action, ActionId, ActionWithMeta, State, Store};

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

fn stats_message_write_start(
    _: &State,
    stats_service: Option<&mut StatisticsService>,
    message: &PeerMessage,
    action_id: ActionId,
) {
    stats_service.map(|stats| {
        let time: u64 = action_id.into();
        match message {
            PeerMessage::GetBlockHeaders(m) => m.get_block_headers().iter().for_each(|b| {
                stats.block_header_download_start(&b, time);
            }),
            PeerMessage::GetOperationsForBlocks(m) => m
                .get_operations_for_blocks()
                .iter()
                .for_each(|b| stats.block_operations_download_start(b.block_hash(), time)),
            _ => {}
        }
    });
}

pub fn peer_message_write_effects<S>(store: &mut Store<S>, action: &ActionWithMeta)
where
    S: Service,
{
    match &action.action {
        Action::PeerMessageWriteNext(content) => {
            let peer = match store.state.get().peers.get(&content.address) {
                Some(peer) => match peer.status.as_handshaked() {
                    Some(v) => v,
                    None => return,
                },
                None => return,
            };

            if let PeerBinaryMessageWriteState::Init { .. } = &peer.message_write.current {
                if let Some(next_message) = peer.message_write.queue.front() {
                    stats_message_write_start(
                        store.state.get(),
                        store.service.statistics(),
                        next_message.message(),
                        action.id,
                    );

                    let message_encode_result = next_message.as_bytes();
                    binary_message_write_init(store, content.address, message_encode_result);
                }
            }
        }
        Action::PeerMessageWriteInit(content) => {
            let peer = match store.state.get().peers.get(&content.address) {
                Some(peer) => match peer.status.as_handshaked() {
                    Some(v) => v,
                    None => return,
                },
                None => return,
            };

            if let PeerBinaryMessageWriteState::Init { .. } = &peer.message_write.current {
                stats_message_write_start(
                    store.state.get(),
                    store.service.statistics(),
                    content.message.message(),
                    action.id,
                );

                binary_message_write_init(store, content.address, content.message.as_bytes());
            }
        }
        Action::PeerBinaryMessageWriteReady(content) => {
            let peer = match store.state().peers.get(&content.address) {
                Some(peer) => match peer.status.as_handshaked() {
                    Some(handshaked) => handshaked,
                    None => return,
                },
                None => return,
            };

            if let PeerBinaryMessageWriteState::Ready { .. } = &peer.message_write.current {
                store.dispatch(PeerMessageWriteSuccessAction {
                    address: content.address,
                });
            }
        }
        Action::PeerMessageWriteSuccess(content) => {
            store.dispatch(PeerMessageWriteNextAction {
                address: content.address,
            });
        }
        Action::PeerMessageWriteError(content) => {
            store.dispatch(PeersGraylistAddressAction {
                address: content.address,
            });
        }
        _ => {}
    }
}
