// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use tezos_api::ffi::ProtocolError;
use tezos_protocol_ipc_client::ProtocolServiceError;

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
    BlockApplierApplyProtocolRunnerApplyInitAction,
    BlockApplierApplyProtocolRunnerApplyPendingAction,
    BlockApplierApplyProtocolRunnerApplyRetryAction,
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
        Action::BlockApplierApplyInit(content) => {
            store.dispatch(StorageRequestCreateAction {
                payload: StorageRequestPayload::PrepareApplyBlockData {
                    chain_id: content.chain_id.clone(),
                    block_hash: content.block_hash.clone(),
                },
            });

            let req_id = store.state().storage.requests.last_added_req_id();
            store.dispatch(BlockApplierApplyPrepareDataPendingAction {
                storage_req_id: req_id,
            });
        }
        Action::BlockApplierApplyPrepareDataPending(_) => {
            let block_hash = match store.state.get().block_applier.current.block_hash() {
                Some(v) => v,
                None => return,
            };
            store
                .service
                .statistics()
                .map(|s| s.block_load_data_start(&block_hash, action.time_as_nanos()));
        }
        Action::BlockApplierApplyPrepareDataSuccess(content) => {
            store.service().statistics().map(|s| {
                s.block_load_data_end(
                    &content.block.hash,
                    content.block.header.level(),
                    action.time_as_nanos(),
                )
            });

            store.dispatch(BlockApplierApplyProtocolRunnerApplyInitAction {});
        }
        Action::BlockApplierApplyProtocolRunnerApplyInit(_)
        | Action::BlockApplierApplyProtocolRunnerApplyRetry(_) => {
            let (block_hash, req) = match &store.state.get().block_applier.current {
                BlockApplierApplyState::PrepareDataSuccess {
                    block,
                    apply_block_req,
                    ..
                } => (&block.hash, apply_block_req.clone()),
                BlockApplierApplyState::ProtocolRunnerApplyPending {
                    block,
                    apply_block_req,
                    ..
                } => (&block.hash, apply_block_req.clone()),
                _ => return,
            };

            store
                .service
                .statistics()
                .map(|s| s.block_apply_start(&block_hash, action.time_as_nanos()));

            store.service.protocol_runner().apply_block((*req).clone());
            store.dispatch(BlockApplierApplyProtocolRunnerApplyPendingAction {});
        }
        Action::ProtocolRunnerResponse(content) => {
            let result = match &content.result {
                ProtocolRunnerResult::ApplyBlock((_, res)) => res,
                _ => return,
            };
            match result {
                Ok(result) => store.dispatch(BlockApplierApplyProtocolRunnerApplySuccessAction {
                    apply_result: result.clone().into(),
                }),
                Err(err) => {
                    let block_hash = match &store.state.get().block_applier.current {
                        BlockApplierApplyState::ProtocolRunnerApplyPending { block, .. } => {
                            Some(Arc::new(block.hash.clone()))
                        }
                        _ => None,
                    };

                    if let ProtocolServiceError::ProtocolError {
                        reason: ProtocolError::ApplyBlockError { reason },
                    } = err
                    {
                        if store.dispatch(BlockApplierApplyProtocolRunnerApplyRetryAction {
                            reason: reason.clone(),
                            block_hash: block_hash.clone(),
                        }) {
                            // if retrying is enabled, return, otherwise
                            // dispatch error action.
                            return;
                        }
                    }

                    store.dispatch(BlockApplierApplyErrorAction {
                        error: BlockApplierApplyError::ProtocolRunnerApply {
                            service_error: err.clone(),
                            block_hash,
                        },
                    })
                }
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
                    Arc::new(block.hash.clone()),
                    block_meta.clone(),
                    apply_result.clone(),
                ),
                _ => return,
            };
            store
                .service
                .statistics()
                .map(|s| s.block_apply_end(&block_hash, action.time_as_nanos(), &block_result));
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
        Action::BlockApplierApplyStoreApplyResultPending(_) => {
            let block_hash = match store.state.get().block_applier.current.block_hash() {
                Some(v) => v,
                None => return,
            };
            store
                .service
                .statistics()
                .map(|s| s.block_store_result_start(block_hash, action.time_as_nanos()));
        }
        Action::BlockApplierApplyStoreApplyResultSuccess(_) => {
            let block_hash = match store.state.get().block_applier.current.block_hash() {
                Some(v) => v,
                None => return,
            };
            store
                .service
                .statistics()
                .map(|s| s.block_store_result_end(block_hash, action.time_as_nanos()));
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

        Action::StorageResponseReceived(content) => {
            let expected_req_id = match &state.block_applier.current {
                BlockApplierApplyState::PrepareDataPending { storage_req_id, .. } => {
                    *storage_req_id
                }
                BlockApplierApplyState::StoreApplyResultPending { storage_req_id, .. } => {
                    *storage_req_id
                }
                _ => return,
            };
            if let Some(req_id) = content.response.req_id.clone() {
                if req_id != expected_req_id {
                    return;
                }
            }
            match &content.response.result {
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
