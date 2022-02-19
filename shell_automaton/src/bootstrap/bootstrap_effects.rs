// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::sync::Arc;

use crypto::hash::BlockHash;
use networking::network_channel::{AllBlockOperationsReceived, BlockReceived};
use tezos_messages::p2p::encoding::block_header::GetBlockHeadersMessage;
use tezos_messages::p2p::encoding::operations_for_blocks::{
    GetOperationsForBlocksMessage, OperationsForBlock,
};
use tezos_messages::p2p::encoding::peer::PeerMessage;
use tezos_messages::p2p::encoding::prelude::GetCurrentBranchMessage;

use crate::block_applier::BlockApplierEnqueueBlockAction;
use crate::bootstrap::BootstrapState;
use crate::peer::message::write::PeerMessageWriteInitAction;
use crate::service::actors_service::ActorsMessageTo;
use crate::service::storage_service::StorageRequestPayload;
use crate::service::{ActorsService, RandomnessService};
use crate::storage::request::{StorageRequestCreateAction, StorageRequestor};
use crate::{Action, ActionWithMeta, Service, Store};

use super::{
    BootstrapCheckTimeoutsInitAction, BootstrapPeerBlockHeaderGetFinishAction,
    BootstrapPeerBlockHeaderGetInitAction, BootstrapPeerBlockHeaderGetPendingAction,
    BootstrapPeerBlockHeaderGetTimeoutAction, BootstrapPeerBlockOperationsGetPendingAction,
    BootstrapPeerBlockOperationsGetRetryAction, BootstrapPeerBlockOperationsGetSuccessAction,
    BootstrapPeerBlockOperationsGetTimeoutAction, BootstrapPeersBlockHeadersGetInitAction,
    BootstrapPeersBlockHeadersGetPendingAction, BootstrapPeersBlockHeadersGetSuccessAction,
    BootstrapPeersBlockOperationsGetInitAction, BootstrapPeersBlockOperationsGetNextAction,
    BootstrapPeersBlockOperationsGetNextAllAction, BootstrapPeersBlockOperationsGetPendingAction,
    BootstrapPeersBlockOperationsGetSuccessAction, BootstrapPeersConnectPendingAction,
    BootstrapPeersConnectSuccessAction, BootstrapPeersMainBranchFindInitAction,
    BootstrapPeersMainBranchFindPendingAction, BootstrapPeersMainBranchFindSuccessAction,
    BootstrapScheduleBlockForApplyAction, BootstrapScheduleBlocksForApplyAction,
    PeerIntervalCurrentState,
};

pub fn bootstrap_effects<S>(store: &mut Store<S>, action: &ActionWithMeta)
where
    S: Service,
{
    store.dispatch(BootstrapCheckTimeoutsInitAction {});
    match &action.action {
        Action::BootstrapInit(_) => {
            store.dispatch(BootstrapPeersConnectPendingAction {});
        }
        Action::BootstrapPeersConnectPending(_) => {
            store.dispatch(BootstrapPeersConnectSuccessAction {});
        }
        Action::BootstrapPeersConnectSuccess(_) => {
            store.dispatch(BootstrapPeersMainBranchFindInitAction {});
        }
        Action::PeerHandshakingFinish(content) => match &store.state().bootstrap {
            BootstrapState::PeersConnectPending { .. } => {
                store.dispatch(BootstrapPeersConnectSuccessAction {});
            }
            BootstrapState::PeersMainBranchFindPending { .. }
            | BootstrapState::PeersBlockHeadersGetPending { .. } => {
                let message = GetCurrentBranchMessage::new(store.state().config.chain_id.clone());
                store.dispatch(PeerMessageWriteInitAction {
                    address: content.address,
                    message: Arc::new(PeerMessage::GetCurrentBranch(message).into()),
                });
            }
            _ => {}
        },
        Action::BootstrapPeersMainBranchFindInit(_) => {
            let state = store.state.get();

            let peers = state
                .peers
                .handshaked_iter()
                .map(|(addr, _)| addr)
                .collect::<Vec<_>>();
            let message = GetCurrentBranchMessage::new(state.config.chain_id.clone());

            for peer in peers {
                let message = message.clone();
                store.dispatch(PeerMessageWriteInitAction {
                    address: peer,
                    message: Arc::new(PeerMessage::GetCurrentBranch(message).into()),
                });
            }

            store.dispatch(BootstrapPeersMainBranchFindPendingAction {});
        }
        Action::BootstrapPeerCurrentBranchReceived(content) => {
            if !store.dispatch(BootstrapPeersMainBranchFindSuccessAction {}) {
                store.dispatch(BootstrapPeerBlockHeaderGetInitAction { peer: content.peer });
            }
        }
        Action::BootstrapPeersMainBranchFindSuccess(_) => {
            store.dispatch(BootstrapPeersBlockHeadersGetInitAction {});
        }
        Action::BootstrapPeersBlockHeadersGetInit(_) => {
            store.dispatch(BootstrapPeersBlockHeadersGetPendingAction {});

            request_block_headers_from_available_peers(store);
        }
        Action::BootstrapPeersBlockHeadersGetPending(_) => {
            store.dispatch(BootstrapPeersBlockHeadersGetSuccessAction {});
        }
        Action::BootstrapPeerBlockHeaderGetInit(content) => {
            let bootstrap_state = &store.state().bootstrap;
            let block_hash = match bootstrap_state
                .peer_next_interval(content.peer)
                .and_then(|(_, p)| p.current.block_hash())
            {
                Some(v) => v.clone(),
                None => return,
            };
            let message = GetBlockHeadersMessage::new(vec![block_hash]);
            store.dispatch(PeerMessageWriteInitAction {
                address: content.peer,
                message: Arc::new(PeerMessage::GetBlockHeaders(message).into()),
            });
            store.dispatch(BootstrapPeerBlockHeaderGetPendingAction { peer: content.peer });
        }
        Action::BootstrapPeerBlockHeaderGetSuccess(content) => {
            store
                .service
                .actors()
                .send(ActorsMessageTo::BlockReceived(BlockReceived {
                    hash: content.block.hash.clone(),
                    level: content.block.header.level(),
                }));

            let chain_id = store.state().config.chain_id.clone();
            store.dispatch(StorageRequestCreateAction {
                payload: StorageRequestPayload::BlockHeaderPut(chain_id, content.block.clone()),
                requestor: StorageRequestor::Bootstrap,
            });

            store.dispatch(BootstrapPeerBlockHeaderGetFinishAction { peer: content.peer });
        }
        Action::BootstrapPeerBlockHeaderGetFinish(content) => {
            store.dispatch(BootstrapPeerBlockHeaderGetInitAction { peer: content.peer });
            store.dispatch(BootstrapPeersBlockHeadersGetSuccessAction {});
        }
        Action::BootstrapPeersBlockHeadersGetSuccess(_) => {
            store.dispatch(BootstrapPeersBlockOperationsGetInitAction {});
        }
        Action::BootstrapPeersBlockOperationsGetInit(_) => {
            store.dispatch(BootstrapPeersBlockOperationsGetPendingAction {});
        }
        Action::BootstrapPeersBlockOperationsGetPending(_) => {
            store.dispatch(BootstrapPeersBlockOperationsGetNextAllAction {});
            store.dispatch(BootstrapPeersBlockOperationsGetSuccessAction {});
        }
        Action::BootstrapPeersBlockOperationsGetNextAll(_) => {
            for _ in 0..2 {
                if !store.dispatch(BootstrapPeersBlockOperationsGetNextAction {}) {
                    break;
                }
            }
        }
        Action::BootstrapPeersBlockOperationsGetNext(_) => {
            let (peer, block_hash, validation_pass) =
                match store.state().bootstrap.operations_get_queue_next() {
                    Some(v) => (v.peer, v.block_hash.clone(), v.validation_pass),
                    None => return,
                };

            let peer = match peer
                .filter(|peer| store.state().peers.get_handshaked(peer).is_some())
                .or_else(|| {
                    let handshaked_iter = store.state().peers.handshaked_iter();
                    let peers = handshaked_iter.map(|(addr, _)| addr).collect::<Vec<_>>();
                    store.service.randomness().choose_peer(&peers)
                }) {
                Some(v) => v,
                // TODO(zura): log that we dont have peers for getting ops.
                None => return,
            };

            request_block_operations(store, peer, block_hash, validation_pass);
        }
        Action::BootstrapPeerBlockOperationsGetRetry(content) => {
            let validation_pass = match &store.state().bootstrap {
                BootstrapState::PeersBlockOperationsGetPending { pending, .. } => {
                    match pending.get(&content.block_hash) {
                        Some(v) => v.validation_pass,
                        None => return,
                    }
                }
                _ => return,
            };
            request_block_operations(
                store,
                content.peer.clone(),
                content.block_hash.clone(),
                validation_pass,
            );
        }
        Action::BootstrapPeerBlockOperationsReceived(content) => {
            store.dispatch(BootstrapPeerBlockOperationsGetSuccessAction {
                block_hash: content.message.operations_for_block().block_hash().clone(),
            });
        }
        Action::BootstrapPeerBlockOperationsGetSuccess(content) => {
            let state = store.state.get();
            let operations_list = match state
                .bootstrap
                .operations_get_completed(&content.block_hash)
            {
                Some(v) => v,
                None => return,
            };
            let level = match &state.bootstrap {
                BootstrapState::PeersBlockOperationsGetPending { pending, .. } => {
                    match pending.get(&content.block_hash) {
                        Some(v) => v.block_level,
                        None => return,
                    }
                }
                _ => return,
            };
            store
                .service
                .actors()
                .send(ActorsMessageTo::AllBlockOperationsReceived(
                    AllBlockOperationsReceived { level },
                ));
            for operations in operations_list.clone() {
                store.dispatch(StorageRequestCreateAction {
                    payload: StorageRequestPayload::BlockOperationsPut(operations),
                    requestor: StorageRequestor::Bootstrap,
                });
            }
            store.dispatch(BootstrapScheduleBlocksForApplyAction {});
            store.dispatch(BootstrapPeersBlockOperationsGetSuccessAction {});
        }
        Action::BootstrapScheduleBlocksForApply(_) => loop {
            let block_hash = match store.state().bootstrap.next_block_for_apply() {
                Some(v) => v.clone(),
                None => break,
            };
            if !store.dispatch(BootstrapScheduleBlockForApplyAction { block_hash }) {
                break;
            }
        },
        Action::BootstrapScheduleBlockForApply(content) => {
            store.dispatch(BlockApplierEnqueueBlockAction {
                block_hash: content.block_hash.clone().into(),
                injector_rpc_id: None,
            });
            store.dispatch(BootstrapPeersBlockOperationsGetNextAllAction {});
            store.dispatch(BootstrapPeersBlockOperationsGetSuccessAction {});
        }
        Action::CurrentHeadUpdate(_) => {
            store.dispatch(BootstrapPeersBlockOperationsGetNextAllAction {});
            store.dispatch(BootstrapScheduleBlocksForApplyAction {});
        }
        Action::BootstrapPeerBlockHeaderGetTimeout(content) => {
            request_block_headers_from_available_peers(store);
        }
        Action::BootstrapPeerBlockOperationsGetTimeout(content) => {
            retry_block_operations_request(store, content.block_hash.clone());
        }
        Action::PeerDisconnected(content) => {
            let state = store.state.get();
            match &state.bootstrap {
                BootstrapState::PeersBlockHeadersGetPending { .. } => {
                    request_block_headers_from_available_peers(store);
                }
                BootstrapState::PeersBlockOperationsGetPending { pending, .. } => {
                    let blocks = pending
                        .iter()
                        .filter(|(_, b)| {
                            b.peers
                                .get(&content.address)
                                .map(|p| p.is_pending())
                                .unwrap_or(false)
                        })
                        .map(|(block_hash, _)| block_hash.clone())
                        .collect::<Vec<_>>();
                    for block_hash in blocks {
                        retry_block_operations_request(store, block_hash);
                    }
                }
                _ => {}
            }
        }
        Action::BootstrapCheckTimeoutsInit(_) => {
            let state = store.state.get();
            match &state.bootstrap {
                BootstrapState::PeersBlockHeadersGetPending { peer_intervals, .. } => {
                    let current_time = state.time_as_nanos();
                    let timeout = state.config.bootstrap_block_header_get_timeout.as_nanos() as u64;
                    let timeouts = peer_intervals
                        .iter()
                        .filter(|p| p.current.is_pending_timed_out(timeout, current_time))
                        .filter_map(|p| match &p.current {
                            PeerIntervalCurrentState::Pending {
                                peer, block_hash, ..
                            } => Some((*peer, block_hash.clone())),
                            _ => None,
                        })
                        .collect::<Vec<_>>();

                    for (peer, block_hash) in timeouts {
                        store.dispatch(BootstrapPeerBlockHeaderGetTimeoutAction {
                            peer,
                            block_hash,
                        });
                    }
                }
                BootstrapState::PeersBlockOperationsGetPending { pending, .. } => {
                    let current_time = state.time_as_nanos();
                    let timeout = state
                        .config
                        .bootstrap_block_operations_get_timeout
                        .as_nanos() as u64;
                    let timeouts = pending
                        .iter()
                        .flat_map(|(block_hash, b)| {
                            b.peers
                                .iter()
                                .filter(|(_, p)| p.is_pending_timed_out(timeout, current_time))
                                .map(move |(peer, _)| (*peer, block_hash.clone()))
                        })
                        .collect::<Vec<_>>();

                    for (peer, block_hash) in timeouts {
                        store.dispatch(BootstrapPeerBlockOperationsGetTimeoutAction {
                            peer,
                            block_hash,
                        });
                    }
                }
                _ => {}
            }
        }
        _ => {}
    }
}

pub fn request_block_headers_from_available_peers<S>(store: &mut Store<S>)
where
    S: Service,
{
    let state = store.state.get();
    let peers = state
        .peers
        .handshaked_iter()
        .map(|(addr, _)| addr)
        .collect::<BTreeSet<_>>();
    for peer in peers {
        store.dispatch(BootstrapPeerBlockHeaderGetInitAction { peer });
    }
}

pub fn retry_block_operations_request<S>(store: &mut Store<S>, block_hash: BlockHash)
where
    S: Service,
{
    // peers that we have already tried to get these operations from.
    let existing_peers = match &store.state().bootstrap {
        BootstrapState::PeersBlockOperationsGetPending { pending, .. } => {
            match pending.get(&block_hash) {
                Some(v) => v
                    .peers
                    .iter()
                    .map(|(addr, _)| *addr)
                    .collect::<BTreeSet<_>>(),
                None => return,
            }
        }
        _ => return,
    };
    let peers = store
        .state()
        .peers
        .handshaked_iter()
        .map(|(addr, _)| addr)
        .collect::<Vec<_>>();
    let new_peers = peers
        .iter()
        .map(|p| *p)
        .filter(|p| !existing_peers.contains(p))
        .collect::<Vec<_>>();
    let peers = if new_peers.is_empty() {
        peers
    } else {
        new_peers
    };
    let peer = match store.service.randomness().choose_peer(&peers) {
        Some(v) => v,
        // TODO(zura): log.
        None => return,
    };
    store.dispatch(BootstrapPeerBlockOperationsGetRetryAction { peer, block_hash });
}

pub fn request_block_operations<S>(
    store: &mut Store<S>,
    peer: SocketAddr,
    block_hash: BlockHash,
    validation_pass: u8,
) where
    S: Service,
{
    let operations_for_blocks = (0..validation_pass)
        .map(|vp| OperationsForBlock::new(block_hash.clone(), vp as i8))
        .collect();
    let message = GetOperationsForBlocksMessage::new(operations_for_blocks);
    store.dispatch(PeerMessageWriteInitAction {
        address: peer,
        message: Arc::new(PeerMessage::GetOperationsForBlocks(message).into()),
    });
    store.dispatch(BootstrapPeerBlockOperationsGetPendingAction {
        peer,
        block_hash: block_hash.clone(),
    });
}
