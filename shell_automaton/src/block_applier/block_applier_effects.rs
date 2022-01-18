// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::service::protocol_runner_service::ProtocolRunnerResult;
use crate::service::storage_service::{
    StorageRequestPayload, StorageResponseError, StorageResponseSuccess,
};
use crate::service::{ActorsService, ProtocolRunnerService};
use crate::storage::request::StorageRequestCreateAction;
use crate::{Action, ActionWithMeta, Service, Store};

use super::{
    BlockApplierApplyError, BlockApplierApplyErrorAction, BlockApplierApplyInitAction,
    BlockApplierApplyPrepareDataPendingAction, BlockApplierApplyPrepareDataSuccessAction,
    BlockApplierApplyProtocolRunnerApplyPendingAction,
    BlockApplierApplyProtocolRunnerApplySuccessAction, BlockApplierApplyState,
    BlockApplierApplyStoreApplyResultPendingAction, BlockApplierApplyStoreApplyResultSuccessAction,
    BlockApplierApplySuccessAction,
};

pub fn block_applier_effects<S>(store: &mut Store<S>, action: &ActionWithMeta)
where
    S: Service,
{
    let state = store.state.get();

    match &action.action {
        Action::BlockApplierApplyInit(action) => {
            store.dispatch(StorageRequestCreateAction {
                payload: StorageRequestPayload::PrepareApplyBlockData {
                    chain_id: action.chain_id.clone(),
                    block_hash: action.block_hash.clone(),
                },
            });

            let req_id = store.state().storage.requests.last_added_req_id();
            store.dispatch(BlockApplierApplyPrepareDataPendingAction {
                storage_req_id: req_id,
            });
        }
        Action::StorageResponseReceived(action) => {
            let expected_req_id = match &state.block_applier.current {
                BlockApplierApplyState::PrepareDataPending { storage_req_id, .. } => {
                    *storage_req_id
                }
                BlockApplierApplyState::StoreApplyResultPending { storage_req_id, .. } => {
                    *storage_req_id
                }
                _ => return,
            };
            if let Some(req_id) = action.response.req_id.clone() {
                if req_id != expected_req_id {
                    return;
                }
            }
            match &action.response.result {
                Ok(StorageResponseSuccess::PrepareApplyBlockDataSuccess {
                    block,
                    block_meta,
                    apply_block_req,
                }) => {
                    store.dispatch(BlockApplierApplyPrepareDataSuccessAction {
                        block: block.clone(),
                        block_meta: block_meta.clone(),
                        apply_block_req: apply_block_req.clone(),
                    });
                }
                Err(StorageResponseError::PrepareApplyBlockDataError(err)) => {
                    store.dispatch(BlockApplierApplyErrorAction {
                        error: BlockApplierApplyError::PrepareData(err.clone()),
                    });
                }
                Ok(StorageResponseSuccess::StoreApplyBlockResultSuccess(data)) => {
                    store.dispatch(BlockApplierApplyStoreApplyResultSuccessAction {
                        block_additional_data: data.clone(),
                    });
                }
                Err(StorageResponseError::StoreApplyBlockResultError(err)) => {
                    store.dispatch(BlockApplierApplyErrorAction {
                        error: BlockApplierApplyError::StoreApplyResult(err.clone()),
                    });
                }
                _ => return,
            }
        }
        Action::BlockApplierApplyPrepareDataSuccess(_) => {
            let req = match &store.state().block_applier.current {
                BlockApplierApplyState::PrepareDataSuccess {
                    apply_block_req, ..
                } => apply_block_req.clone(),
                _ => return,
            };
            store.service.protocol_runner().apply_block((*req).clone());
            store.dispatch(BlockApplierApplyProtocolRunnerApplyPendingAction {});
        }
        Action::ProtocolRunnerResponse(action) => {
            let result = match &action.result {
                ProtocolRunnerResult::ApplyBlock((_, res)) => res,
                _ => return,
            };
            match result {
                Ok(result) => store.dispatch(BlockApplierApplyProtocolRunnerApplySuccessAction {
                    apply_result: result.clone().into(),
                }),
                Err(err) => store.dispatch(BlockApplierApplyErrorAction {
                    error: BlockApplierApplyError::ProtocolRunnerApply(err.clone()),
                }),
            };
        }
        Action::BlockApplierApplyProtocolRunnerApplySuccess(_) => {
            let (block_hash, block_metadata, block_result) = match &state.block_applier.current {
                BlockApplierApplyState::ProtocolRunnerApplySuccess {
                    block,
                    block_meta,
                    apply_result,
                    ..
                } => (
                    block.hash.clone().into(),
                    block_meta.clone(),
                    apply_result.clone(),
                ),
                _ => return,
            };
            store.dispatch(StorageRequestCreateAction {
                payload: StorageRequestPayload::StoreApplyBlockResult {
                    block_hash,
                    block_metadata,
                    block_result,
                },
            });

            let req_id = store.state().storage.requests.last_added_req_id();
            store.dispatch(BlockApplierApplyStoreApplyResultPendingAction {
                storage_req_id: req_id,
            });
        }
        Action::BlockApplierApplyStoreApplyResultSuccess(_) => {
            store.dispatch(BlockApplierApplySuccessAction {});
        }
        Action::BlockApplierEnqueueBlock(_) => {
            start_applying_next_block(store);
        }
        Action::BlockApplierApplySuccess(_) => {
            match &store.state.get().block_applier.current {
                BlockApplierApplyState::Success {
                    chain_id, block, ..
                } => {
                    store.service.actors().call_apply_block_callback(
                        &block.hash,
                        Ok((chain_id.clone(), block.clone())),
                    );
                }
                _ => return,
            }
            start_applying_next_block(store);
        }
        _ => {}
    }
}

pub fn start_applying_next_block<S: Service>(store: &mut Store<S>) {
    if let Some((chain_id, block_hash)) = store.state().block_applier.queue.front().cloned() {
        store.dispatch(BlockApplierApplyInitAction {
            chain_id,
            block_hash,
        });
    }
}
