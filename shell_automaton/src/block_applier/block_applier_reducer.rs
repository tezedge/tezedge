// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::{Action, ActionWithMeta, State};

use super::BlockApplierApplyState;

pub fn block_applier_reducer(state: &mut State, action: &ActionWithMeta) {
    match &action.action {
        Action::BlockApplierEnqueueBlock(content) => {
            state
                .block_applier
                .queue
                .push_back((content.block_hash.clone(), content.injector_rpc_id.clone()));
        }
        Action::BlockApplierApplyInit(content) => {
            if let Some((block_hash, _)) = state.block_applier.queue.front() {
                if block_hash == &content.block_hash {
                    state.block_applier.queue.pop_front();
                }
            }
            state.block_applier.current = BlockApplierApplyState::Init {
                time: action.time_as_nanos(),
                block_hash: content.block_hash.clone(),
                injector_rpc_id: content.injector_rpc_id.clone(),
            };
        }
        Action::BlockApplierApplyPrepareDataPending(content) => {
            match &state.block_applier.current {
                BlockApplierApplyState::Init {
                    block_hash,
                    injector_rpc_id,
                    ..
                } => {
                    state.block_applier.current = BlockApplierApplyState::PrepareDataPending {
                        time: action.time_as_nanos(),
                        storage_req_id: content.storage_req_id,
                        block_hash: block_hash.clone(),
                        injector_rpc_id: injector_rpc_id.clone(),
                    };
                }
                _ => {}
            };
        }
        Action::BlockApplierApplyPrepareDataSuccess(content) => {
            match &state.block_applier.current {
                BlockApplierApplyState::PrepareDataPending {
                    time,
                    injector_rpc_id,
                    ..
                } => {
                    state.block_applier.current = BlockApplierApplyState::PrepareDataSuccess {
                        time: action.time_as_nanos(),
                        prepare_data_duration: action.time_as_nanos() - time,
                        block: content.block.clone(),
                        block_meta: content.block_meta.clone(),
                        apply_block_req: content.apply_block_req.clone(),
                        injector_rpc_id: injector_rpc_id.clone(),
                    };
                }
                _ => {}
            };
        }
        Action::BlockApplierApplyProtocolRunnerApplyPending(_) => {
            match &state.block_applier.current {
                BlockApplierApplyState::PrepareDataSuccess {
                    prepare_data_duration,
                    block,
                    block_meta,
                    apply_block_req,
                    injector_rpc_id,
                    ..
                } => {
                    state.block_applier.current =
                        BlockApplierApplyState::ProtocolRunnerApplyPending {
                            time: action.time_as_nanos(),
                            prepare_data_duration: prepare_data_duration.clone(),
                            block: block.clone(),
                            block_meta: block_meta.clone(),
                            apply_block_req: apply_block_req.clone(),

                            retry: None,
                            injector_rpc_id: injector_rpc_id.clone(),
                        };
                }
                _ => return,
            };
        }
        Action::BlockApplierApplyProtocolRunnerApplyRetry(content) => {
            match &mut state.block_applier.current {
                BlockApplierApplyState::ProtocolRunnerApplyPending { retry, .. } => {
                    *retry = Some(content.reason.clone());
                }
                _ => return,
            };
        }
        Action::BlockApplierApplyProtocolRunnerApplySuccess(content) => {
            match &state.block_applier.current {
                BlockApplierApplyState::ProtocolRunnerApplyPending {
                    time,
                    prepare_data_duration,
                    block,
                    block_meta,
                    retry,
                    injector_rpc_id,
                    ..
                } => {
                    state.block_applier.current =
                        BlockApplierApplyState::ProtocolRunnerApplySuccess {
                            time: action.time_as_nanos(),
                            prepare_data_duration: *prepare_data_duration,
                            protocol_runner_apply_duration: action.time_as_nanos() - time,
                            block: block.clone(),
                            block_meta: block_meta.clone(),
                            apply_result: content.apply_result.clone(),
                            retry: retry.clone(),
                            injector_rpc_id: injector_rpc_id.clone(),
                        };
                }
                _ => return,
            };
        }
        Action::BlockApplierApplyStoreApplyResultPending(content) => {
            match &state.block_applier.current {
                BlockApplierApplyState::ProtocolRunnerApplySuccess {
                    prepare_data_duration,
                    protocol_runner_apply_duration,
                    block,
                    block_meta,
                    apply_result,
                    retry,
                    injector_rpc_id,
                    ..
                } => {
                    state.block_applier.current = BlockApplierApplyState::StoreApplyResultPending {
                        time: action.time_as_nanos(),
                        prepare_data_duration: *prepare_data_duration,
                        protocol_runner_apply_duration: *protocol_runner_apply_duration,
                        storage_req_id: content.storage_req_id.clone(),
                        block: block.clone(),
                        block_meta: block_meta.clone(),
                        apply_result: apply_result.clone(),
                        retry: retry.clone(),
                        injector_rpc_id: injector_rpc_id.clone(),
                    };
                }
                _ => return,
            };
        }
        Action::BlockApplierApplyStoreApplyResultSuccess(content) => {
            match &state.block_applier.current {
                BlockApplierApplyState::StoreApplyResultPending {
                    time,
                    prepare_data_duration,
                    protocol_runner_apply_duration,
                    block,
                    apply_result,
                    retry,
                    injector_rpc_id,
                    ..
                } => {
                    state.block_applier.current = BlockApplierApplyState::StoreApplyResultSuccess {
                        time: action.time_as_nanos(),
                        prepare_data_duration: *prepare_data_duration,
                        protocol_runner_apply_duration: *protocol_runner_apply_duration,
                        store_apply_result_duration: action.time_as_nanos() - time,
                        block: block.clone(),
                        block_additional_data: content.block_additional_data.clone(),
                        apply_result: apply_result.clone(),
                        retry: retry.clone(),
                        injector_rpc_id: injector_rpc_id.clone(),
                    };
                }
                _ => return,
            };
        }
        Action::BlockApplierApplyError(content) => {
            let injector_rpc_id = state.block_applier.current.injector_rpc_id();
            let block_hash = match state.block_applier.current.block_hash() {
                Some(v) => v.clone().into(),
                None => return,
            };

            state.block_applier.current = BlockApplierApplyState::Error {
                error: content.error.clone(),
                block_hash,
                injector_rpc_id,
            };
        }
        Action::BlockApplierApplySuccess(_) => {
            match &state.block_applier.current {
                BlockApplierApplyState::StoreApplyResultSuccess {
                    time,
                    prepare_data_duration,
                    protocol_runner_apply_duration,
                    store_apply_result_duration,
                    block,
                    block_additional_data,
                    apply_result,
                    retry,
                    injector_rpc_id,
                } => {
                    state.block_applier.last_applied = block.hash.clone().into();
                    state.block_applier.current = BlockApplierApplyState::Success {
                        time: time.clone(),
                        prepare_data_duration: prepare_data_duration.clone(),
                        protocol_runner_apply_duration: protocol_runner_apply_duration.clone(),
                        store_apply_result_duration: store_apply_result_duration.clone(),
                        block: block.clone(),
                        block_additional_data: block_additional_data.clone(),
                        apply_result: apply_result.clone(),
                        retry: retry.clone(),
                        injector_rpc_id: injector_rpc_id.clone(),
                    };
                }
                _ => return,
            };
        }
        _ => {}
    }
}
