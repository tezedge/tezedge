// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use crate::current_head::CurrentHeadUpdateAction;
use crate::service::protocol_runner_service::ProtocolRunnerResult;
use crate::service::storage_service::{
    StorageRequestPayload, StorageResponseError, StorageResponseSuccess,
};
use crate::service::{ActorsService, ProtocolRunnerService, RpcService};
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
            let storage_req_id = store.state().storage.requests.next_req_id();
            store.dispatch(StorageRequestCreateAction {
                payload: StorageRequestPayload::PrepareApplyBlockData {
                    chain_id: chain_id.into(),
                    block_hash: content.block_hash.clone(),
                },
                requestor: StorageRequestor::BlockApplier,
            });

            store.dispatch(BlockApplierApplyPrepareDataPendingAction { storage_req_id });
        }
        Action::BlockApplierApplyPrepareDataPending(_) => {
            let block_hash = match store.state.get().block_applier.current.block_hash() {
                Some(v) => v,
                None => return,
            };
            if let Some(s) = store.service.statistics() {
                s.block_load_data_start(block_hash, action.time_as_nanos())
            }
        }
        Action::BlockApplierApplyPrepareDataSuccess(content) => {
            let start_time = match &store.state().block_applier.current {
                BlockApplierApplyState::PrepareDataSuccess {
                    time,
                    prepare_data_duration,
                    ..
                } => time.saturating_sub(*prepare_data_duration),
                _ => return,
            };
            if let Some(s) = store.service().statistics() {
                if !s.block_stats_get_all().contains_key(&content.block.hash) {
                    s.block_new(
                        content.block.hash.clone(),
                        content.block.header.level(),
                        content.block.header.timestamp().into(),
                        content.block.header.validation_pass(),
                        content.block.header.fitness().round(),
                        start_time,
                        None,
                        None,
                        None,
                    );
                }
                s.block_load_data_start(&content.block.hash, start_time);
                s.block_load_data_end(
                    &content.block.hash,
                    content.block.header.level(),
                    action.time_as_nanos(),
                )
            }

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

            if let Some(s) = store.service.statistics() {
                s.block_apply_start(block_hash, action.time_as_nanos())
            }

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
            if let Some(s) = store.service.statistics() {
                s.block_apply_end(&block_hash, action.time_as_nanos(), &block_result)
            }

            let storage_req_id = store.state().storage.requests.next_req_id();
            store.dispatch(StorageRequestCreateAction {
                payload: StorageRequestPayload::StoreApplyBlockResult {
                    block_hash,
                    block_fitness,
                    block_metadata,
                    block_result,
                },
                requestor: StorageRequestor::BlockApplier,
            });

            store.dispatch(BlockApplierApplyStoreApplyResultPendingAction { storage_req_id });
        }
        Action::BlockApplierApplyStoreApplyResultPending(_) => {
            let block_hash = match store.state.get().block_applier.current.block_hash() {
                Some(v) => v,
                None => return,
            };
            if let Some(s) = store.service.statistics() {
                s.block_store_result_start(block_hash, action.time_as_nanos())
            }
        }
        Action::BlockApplierApplyStoreApplyResultSuccess(_) => {
            let block_hash = match store.state.get().block_applier.current.block_hash() {
                Some(v) => v,
                None => return,
            };
            if let Some(s) = store.service.statistics() {
                s.block_store_result_end(block_hash, action.time_as_nanos())
            }
            store.dispatch(BlockApplierApplySuccessAction {});
        }
        Action::BlockApplierApplyError(_) => {
            match &store.state.get().block_applier.current {
                BlockApplierApplyState::Error {
                    injector_rpc_id,
                    error,
                    ..
                } => {
                    if let Some(rpc_id) = *injector_rpc_id {
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
                    block_additional_data,
                    payload_hash,
                    pred_block_metadata_hash,
                    pred_ops_metadata_hash,
                    ..
                } => {
                    let chain_id = store.state().config.chain_id.clone();
                    let block_hash = block.hash.clone();
                    store.service.actors().call_apply_block_callback(
                        &block_hash,
                        Ok((chain_id.into(), block.clone())),
                    );
                    if let Some(rpc_id) = *injector_rpc_id {
                        store.service.rpc().respond(rpc_id, serde_json::Value::Null);
                    }
                    let payload_hash = payload_hash.clone();
                    if let Some(stats) = store.service.statistics() {
                        stats.block_payload_hash(&block_hash, payload_hash.as_ref());
                    }
                    let new_head = (**block).clone();
                    let protocol = block_additional_data.protocol_hash.clone();
                    let next_protocol = block_additional_data.next_protocol_hash.clone();
                    let block_metadata_hash = block_additional_data.block_metadata_hash().clone();
                    let ops_metadata_hash = block_additional_data.ops_metadata_hash().clone();
                    let pred_block_metadata_hash = pred_block_metadata_hash.clone();
                    let pred_ops_metadata_hash = pred_ops_metadata_hash.clone();
                    store.dispatch(CurrentHeadUpdateAction {
                        new_head,
                        protocol,
                        next_protocol,
                        payload_hash,
                        block_metadata_hash,
                        ops_metadata_hash,
                        pred_block_metadata_hash,
                        pred_ops_metadata_hash,
                    });
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
            if let Some(req_id) = content.response.req_id {
                if req_id != expected_req_id {
                    return;
                }
            }
            let ok = match &content.response.result {
                Ok(StorageResponseSuccess::PrepareApplyBlockDataSuccess {
                    block,
                    block_meta,
                    apply_block_req,
                }) => store.dispatch(BlockApplierApplyPrepareDataSuccessAction {
                    block: block.clone(),
                    block_meta: block_meta.clone(),
                    apply_block_req: apply_block_req.clone(),
                }),
                Err(StorageResponseError::PrepareApplyBlockDataError(err)) => {
                    store.dispatch(BlockApplierApplyErrorAction {
                        error: BlockApplierApplyError::PrepareData(err.clone()),
                    })
                }
                Ok(StorageResponseSuccess::StoreApplyBlockResultSuccess(data)) => {
                    store.dispatch(BlockApplierApplyStoreApplyResultSuccessAction {
                        block_additional_data: data.clone(),
                    })
                }
                Err(StorageResponseError::StoreApplyBlockResultError(err)) => {
                    store.dispatch(BlockApplierApplyErrorAction {
                        error: BlockApplierApplyError::StoreApplyResult(err.clone()),
                    })
                }
                _ => false,
            };

            if !ok {
                slog::warn!(&store.state().log, "BlockApplier - unexpected storage response";
                    "block_applier_state" => format!("{:?}", store.state().block_applier),
                    "storage_response" => format!("{:?}", content.response));
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
