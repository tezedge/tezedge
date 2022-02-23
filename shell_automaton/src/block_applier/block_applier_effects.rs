// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use crate::current_head::CurrentHeadUpdateAction;
use crate::service::protocol_runner_service::ProtocolRunnerResult;
use crate::service::storage_service::{
    StorageRequestPayload, StorageResponseError, StorageResponseSuccess,
};
use crate::service::{ProtocolRunnerService, RpcService};
use crate::storage::request::{StorageRequestCreateAction, StorageRequestor};
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
    match &action.action {
        Action::BlockApplierEnqueueBlock(_) => {
            start_applying_next_block(store);
        }
        Action::BlockApplierApplyInit(content) => {
            let chain_id = store.state().config.chain_id.clone();
            store.dispatch(StorageRequestCreateAction {
                payload: StorageRequestPayload::PrepareApplyBlockData {
                    chain_id: chain_id.into(),
                    block_hash: content.block_hash.clone(),
                },
                requestor: StorageRequestor::BlockApplier,
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
                // TODO: this should not fail silently, it would be a serious bug if this happens
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

                    if store.dispatch(BlockApplierApplyProtocolRunnerApplyRetryAction {
                        reason: err.clone(),
                        block_hash: block_hash.clone(),
                    }) {
                        // if retrying is enabled, return, otherwise
                        // dispatch error action.
                        return;
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
            let (block_hash, block_fitness, block_metadata, block_result) =
                match &store.state().block_applier.current {
                    BlockApplierApplyState::ProtocolRunnerApplySuccess {
                        block,
                        block_meta,
                        apply_result,
                        ..
                    } => (
                        Arc::new(block.hash.clone()),
                        block.header.fitness().clone(),
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
                    block_fitness,
                    block_metadata,
                    block_result,
                },
                requestor: StorageRequestor::BlockApplier,
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
        Action::BlockApplierApplyError(_) => {
            match &store.state.get().block_applier.current {
                BlockApplierApplyState::Error {
                    injector_rpc_id,
                    error,
                    ..
                } => {
                    if let Some(rpc_id) = injector_rpc_id.clone() {
                        let err_str = format!("{:?}", error);
                        store
                            .service
                            .rpc()
                            .respond(rpc_id, serde_json::Value::String(err_str));
                    }
                }
                _ => return,
            }
            start_applying_next_block(store);
        }
        Action::BlockApplierApplySuccess(_) => {
            match &store.state.get().block_applier.current {
                BlockApplierApplyState::Success {
                    block,
                    injector_rpc_id,
                    ..
                } => {
                    if let Some(rpc_id) = injector_rpc_id.clone() {
                        store.service.rpc().respond(rpc_id, serde_json::Value::Null);
                    }
                    let new_head = (**block).clone();
                    store.dispatch(CurrentHeadUpdateAction { new_head });
                }
                _ => return,
            }
            start_applying_next_block(store);
        }

        Action::StorageResponseReceived(content) => {
            let expected_req_id = match &store.state().block_applier.current {
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
    if let Some((block_hash, injector_rpc_id)) = store.state().block_applier.queue.front().cloned()
    {
        store.dispatch(BlockApplierApplyInitAction {
            block_hash,
            injector_rpc_id,
        });
    }
}
