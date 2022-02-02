// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use networking::network_channel::PeerMessageReceived;
use tezos_messages::p2p::binary_message::{BinaryRead, MessageHash};
use tezos_messages::p2p::encoding::peer::{PeerMessage, PeerMessageResponse};
use tezos_messages::p2p::encoding::prelude::AdvertiseMessage;

use crate::peer::binary_message::read::PeerBinaryMessageReadInitAction;
use crate::peer::message::read::PeerMessageReadErrorAction;
use crate::peer::message::write::PeerMessageWriteInitAction;
use crate::peers::add::multi::PeersAddMultiAction;
use crate::peers::graylist::PeersGraylistAddressAction;
use crate::service::actors_service::{ActorsMessageTo, ActorsService};
use crate::service::{RandomnessService, Service, StatisticsService};
use crate::{Action, ActionId, ActionWithMeta, State, Store};

use super::{PeerMessageReadInitAction, PeerMessageReadSuccessAction};

fn stats_message_received(
    state: &State,
    stats_service: Option<&mut StatisticsService>,
    message: &PeerMessage,
    action_id: ActionId,
) {
    stats_service.map(|stats| {
        let time: u64 = action_id.into();
        let current_head = state.mempool.local_head_state.as_ref();
        let pending_block_header_requests = &state.peers.pending_block_header_requests;
        match message {
            PeerMessage::CurrentHead(m) => {
                let pred = m.current_block_header().predecessor();
                if current_head.filter(|v| v.hash.eq(pred)).is_none() {
                    return;
                }
                m.current_block_header()
                    .message_typed_hash()
                    .map(|b| {
                        if stats.block_stats_get_by_hash(&b).is_none() {
                            stats.block_new(b.clone().into());
                        }
                        stats.block_seen(&b, time);
                    })
                    .unwrap_or(());
            }
            PeerMessage::BlockHeader(m) => m
                .block_header()
                .message_typed_hash()
                .map(|b| {
                    let pred = m.block_header().predecessor();
                    if current_head.filter(|v| v.hash.eq(pred)).is_some() {
                        if stats.block_stats_get_by_hash(&b).is_none() {
                            stats.block_new(b.clone().into());
                        }
                    }
                    if let Some(time) = pending_block_header_requests.get(&b) {
                        stats.block_header_download_start(&b, *time);
                    }
                    stats.block_header_download_end(&b, time);
                })
                .unwrap_or(()),
            PeerMessage::OperationsForBlocks(m) => {
                let block_hash = m.operations_for_block().block_hash();
                stats.block_operations_download_end(block_hash, time);
            }
            _ => {}
        }
    });
}

pub fn peer_message_read_effects<S>(store: &mut Store<S>, action: &ActionWithMeta)
where
    S: Service,
{
    match &action.action {
        Action::PeerMessageReadInit(action) => {
            store.dispatch(PeerBinaryMessageReadInitAction {
                address: action.address,
            });
        }
        Action::PeerBinaryMessageReadReady(action) => {
            match store.state().peers.get(&action.address) {
                Some(peer) => match peer.status.as_handshaked() {
                    Some(_handshaked) => (),
                    None => return,
                },
                None => return,
            };

            match PeerMessageResponse::from_bytes(&action.message) {
                Ok(mut message) => {
                    // Set size hint to unencrypted encoded message size.
                    // Maybe we should set encrypted size instead? Since
                    // that's the actual size of data transmitted.
                    message.set_size_hint(action.message.len());

                    store.dispatch(PeerMessageReadSuccessAction {
                        address: action.address,
                        message: message.into(),
                    });
                }
                Err(err) => {
                    store.dispatch(PeerMessageReadErrorAction {
                        address: action.address,
                        error: err.into(),
                    });
                }
            }
        }
        Action::PeerMessageReadSuccess(content) => {
            store
                .service()
                .actors()
                .send(ActorsMessageTo::PeerMessageReceived(PeerMessageReceived {
                    peer_address: content.address,
                    message: content.message.clone(),
                }));

            match &content.message.message() {
                PeerMessage::Bootstrap => {
                    let potential_peers =
                        store.state.get().peers.potential_iter().collect::<Vec<_>>();
                    let advertise_peers = store
                        .service
                        .randomness()
                        .choose_potential_peers_for_advertise(&potential_peers);
                    store.dispatch(PeerMessageWriteInitAction {
                        address: content.address,
                        message: PeerMessageResponse::from(AdvertiseMessage::new(advertise_peers))
                            .into(),
                    });
                }
                PeerMessage::Advertise(msg) => {
                    store.dispatch(PeersAddMultiAction {
                        addresses: msg.id().iter().filter_map(|x| x.parse().ok()).collect(),
                    });
                }
                _ => {}
            }

            stats_message_received(
                store.state.get(),
                store.service.statistics(),
                content.message.message(),
                action.id,
            );

            // try to read next message.
            store.dispatch(PeerMessageReadInitAction {
                address: content.address,
            });
        }
        Action::PeerMessageReadError(action) => {
            store.dispatch(PeersGraylistAddressAction {
                address: action.address,
            });
        }
        _ => {}
    }
}
