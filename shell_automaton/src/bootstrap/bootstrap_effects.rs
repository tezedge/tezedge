// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeSet;
use std::sync::Arc;

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
use crate::service::ActorsService;
use crate::storage::request::{StorageRequestCreateAction, StorageRequestor};
use crate::{Action, ActionWithMeta, Service, Store};

use super::{
    BootstrapPeerBlockHeaderGetFinishAction, BootstrapPeerBlockHeaderGetInitAction,
    BootstrapPeerBlockHeaderGetPendingAction, BootstrapPeerBlockOperationsGetPendingAction,
    BootstrapPeerBlockOperationsGetSuccessAction, BootstrapPeersBlockHeadersGetInitAction,
    BootstrapPeersBlockHeadersGetPendingAction, BootstrapPeersBlockHeadersGetSuccessAction,
    BootstrapPeersBlockOperationsGetInitAction, BootstrapPeersBlockOperationsGetNextAction,
    BootstrapPeersBlockOperationsGetNextAllAction, BootstrapPeersBlockOperationsGetPendingAction,
    BootstrapPeersBlockOperationsGetSuccessAction, BootstrapPeersConnectPendingAction,
    BootstrapPeersConnectSuccessAction, BootstrapPeersMainBranchFindInitAction,
    BootstrapPeersMainBranchFindPendingAction, BootstrapPeersMainBranchFindSuccessAction,
    BootstrapScheduleBlockForApplyAction, BootstrapScheduleBlocksForApplyAction,
};

pub fn bootstrap_effects<S>(store: &mut Store<S>, action: &ActionWithMeta)
where
    S: Service,
{
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
            BootstrapState::PeersMainBranchFindPending { .. } => {
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

            let bootstrap_state = &store.state.get().bootstrap;
            if let BootstrapState::PeersBlockHeadersGetPending { peer_intervals, .. } =
                bootstrap_state
            {
                let peers = peer_intervals
                    .iter()
                    .map(|p| p.peer)
                    .collect::<BTreeSet<_>>();
                for peer in peers {
                    store.dispatch(BootstrapPeerBlockHeaderGetInitAction { peer });
                }
            }
        }
        Action::BootstrapPeersBlockHeadersGetPending(_) => {
            store.dispatch(BootstrapPeersBlockHeadersGetSuccessAction {});
        }
        Action::BootstrapPeerBlockHeaderGetInit(content) => {
            let bootstrap_state = &store.state().bootstrap;
            let block_hash = match bootstrap_state
                .peer_interval(content.peer, |p| p.current.is_idle())
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
            while store.dispatch(BootstrapPeersBlockOperationsGetNextAction {}) {}
        }
        Action::BootstrapPeersBlockOperationsGetNext(_) => {
            let (peer, block_hash, validation_pass) =
                match store.state().bootstrap.operations_get_queue_next() {
                    Some(v) => (v.peer, v.block_hash.clone(), v.validation_pass),
                    None => return,
                };

            let operations_for_blocks = (0..(validation_pass.max(0)))
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
        _ => {}
    }
}
