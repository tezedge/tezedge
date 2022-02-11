// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::net::SocketAddr;

use crypto::hash::BlockHash;
use networking::network_channel::PeerMessageReceived;
use storage::BlockHeaderWithHash;
use tezos_messages::p2p::binary_message::{BinaryRead, MessageHash};
use tezos_messages::p2p::encoding::peer::{PeerMessage, PeerMessageResponse};
use tezos_messages::p2p::encoding::prelude::AdvertiseMessage;

use crate::bootstrap::{
    BootstrapPeerBlockHeaderReceivedAction, BootstrapPeerBlockOperationsReceivedAction,
    BootstrapPeerCurrentBranchReceivedAction,
};
use crate::peer::binary_message::read::PeerBinaryMessageReadInitAction;
use crate::peer::message::read::PeerMessageReadErrorAction;
use crate::peer::message::write::PeerMessageWriteInitAction;
use crate::peer::remote_requests::block_header_get::PeerRemoteRequestsBlockHeaderGetEnqueueAction;
use crate::peer::remote_requests::block_operations_get::PeerRemoteRequestsBlockOperationsGetEnqueueAction;
use crate::peer::Peer;
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
    address: SocketAddr,
    action_id: ActionId,
) {
    stats_service.map(|stats| {
        let time: u64 = action_id.into();
        let pending_block_header_requests = &state.peers.pending_block_header_requests;
        let node_id = state
            .peers
            .get(&address)
            .and_then(Peer::public_key_hash)
            .cloned();

        match message {
            PeerMessage::CurrentHead(m) => {
                m.current_block_header()
                    .message_typed_hash()
                    .map(|b: BlockHash| {
                        let block_header = m.current_block_header();
                        stats.block_new(
                            b.clone(),
                            block_header.level(),
                            block_header.timestamp(),
                            block_header.validation_pass(),
                            time,
                            Some(address),
                            node_id,
                            None,
                        );
                    })
                    .unwrap_or(());
            }
            PeerMessage::BlockHeader(m) => m
                .block_header()
                .message_typed_hash()
                .map(|b: BlockHash| {
                    let block_header = m.block_header();
                    stats.block_new(
                        b.clone(),
                        block_header.level(),
                        block_header.timestamp(),
                        block_header.validation_pass(),
                        time,
                        Some(address),
                        node_id,
                        None,
                    );
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
            PeerMessage::GetOperationsForBlocks(m) => {
                for gofb in m.get_operations_for_blocks() {
                    stats.block_get_operations_recv(
                        gofb.block_hash(),
                        time,
                        address,
                        node_id.as_ref(),
                        gofb.validation_pass(),
                    );
                }
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
        Action::PeerMessageReadInit(content) => {
            store.dispatch(PeerBinaryMessageReadInitAction {
                address: content.address,
            });
        }
        Action::PeerBinaryMessageReadReady(content) => {
            match store.state().peers.get(&content.address) {
                Some(peer) => match peer.status.as_handshaked() {
                    Some(_handshaked) => (),
                    None => return,
                },
                None => return,
            };

            match PeerMessageResponse::from_bytes(&content.message) {
                Ok(mut message) => {
                    // Set size hint to unencrypted encoded message size.
                    // Maybe we should set encrypted size instead? Since
                    // that's the actual size of data transmitted.
                    message.set_size_hint(content.message.len());

                    store.dispatch(PeerMessageReadSuccessAction {
                        address: content.address,
                        message: message.into(),
                    });
                }
                Err(err) => {
                    store.dispatch(PeerMessageReadErrorAction {
                        address: content.address,
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
                PeerMessage::CurrentBranch(msg) => {
                    if msg.chain_id() == &store.state().config.chain_id {
                        store.dispatch(BootstrapPeerCurrentBranchReceivedAction {
                            peer: content.address,
                            current_branch: msg.current_branch().clone(),
                        });
                    }
                }
                PeerMessage::GetBlockHeaders(msg) => {
                    for block_hash in msg.get_block_headers() {
                        if !store.dispatch(PeerRemoteRequestsBlockHeaderGetEnqueueAction {
                            address: action.address,
                            block_hash: block_hash.clone(),
                        }) {
                            let state = store.state.get();
                            slog::warn!(&state.log, "Peer - Too many block header requests!";
                                "peer" => format!("{}", action.address),
                                "current_requested_block_headers_len" => msg.get_block_headers().len());
                            break;
                        }
                    }
                }
                PeerMessage::GetOperationsForBlocks(msg) => {
                    for key in msg.get_operations_for_blocks() {
                        if !store.dispatch(PeerRemoteRequestsBlockOperationsGetEnqueueAction {
                            address: action.address,
                            key: key.into(),
                        }) {
                            let state = store.state.get();
                            slog::warn!(&state.log, "Peer - Too many block operations requests!";
                                "peer" => format!("{}", action.address),
                                "current_requested_block_operations_len" => msg.get_operations_for_blocks().len());
                            break;
                        }
                    }
                }
                PeerMessage::BlockHeader(msg) => {
                    let state = store.state.get();
                    let block = match BlockHeaderWithHash::new(msg.block_header().clone()) {
                        Ok(v) => v,
                        Err(err) => {
                            slog::warn!(&state.log, "Failed to hash BlockHeader";
                                "peer" => format!("{}", action.address),
                                "peer_pkh" => format!("{:?}", state.peer_public_key_hash_b58check(action.address)),
                                "block_header" => format!("{:?}", msg.block_header()),
                                "error" => format!("{:?}", err));
                            store.dispatch(PeersGraylistAddressAction {
                                address: content.address,
                            });
                            return;
                        }
                    };
                    if let Some(p) = state
                        .bootstrap
                        .peer_interval_by_level(content.address, block.header.level())
                    {
                        if !p.is_current_hash_eq(&block.hash) {
                            slog::warn!(&state.log, "BlockHeader hash didn't match requested hash";
                                "peer" => format!("{}", content.address),
                                "peer_pkh" => format!("{:?}", state.peer_public_key_hash_b58check(content.address)),
                                "block" => format!("{:?}", block),
                                "expected_hash" => format!("{:?}", p.current));
                            store.dispatch(PeersGraylistAddressAction {
                                address: content.address,
                            });
                            return;
                        }
                        store.dispatch(BootstrapPeerBlockHeaderReceivedAction {
                            peer: content.address,
                            block,
                        });
                    } else {
                        slog::warn!(&state.log, "Received unexpected BlockHeader from peer";
                            "peer" => format!("{}", content.address),
                            "peer_pkh" => format!("{:?}", state.peer_public_key_hash_b58check(content.address)),
                            "block_header" => format!("{:?}", msg.block_header()));
                        store.dispatch(PeersGraylistAddressAction {
                            address: content.address,
                        });
                    }
                }
                PeerMessage::OperationsForBlocks(msg) => {
                    store.dispatch(BootstrapPeerBlockOperationsReceivedAction {
                        peer: content.address,
                        message: msg.clone(),
                    });
                }
                _ => {}
            }

            stats_message_received(
                store.state.get(),
                store.service.statistics(),
                content.message.message(),
                content.address,
                action.id,
            );

            // try to read next message.
            store.dispatch(PeerMessageReadInitAction {
                address: content.address,
            });
        }
        Action::PeerMessageReadError(content) => {
            store.dispatch(PeersGraylistAddressAction {
                address: content.address,
            });
        }
        _ => {}
    }
}
