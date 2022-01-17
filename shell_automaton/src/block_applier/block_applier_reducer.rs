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
                .push_back((content.chain_id.clone(), content.block_hash.clone()));
        }
        Action::BlockApplierApplyInit(content) => {
            if let Some((chain_id, block_hash)) = state.block_applier.queue.front() {
                if chain_id == &content.chain_id && block_hash == &content.block_hash {
                    state.block_applier.queue.pop_front();
                }
            }
            state.block_applier.current = BlockApplierApplyState::Init {
                time: action.time_as_nanos(),
                chain_id: content.chain_id.clone(),
                block_hash: content.block_hash.clone(),
            };
        }
        Action::BlockApplierApplyPrepareDataPending(content) => {
            let (chain_id, block_hash) = match &state.block_applier.current {
                BlockApplierApplyState::Init {
                    chain_id,
                    block_hash,
                    ..
                } => (chain_id, block_hash),
                _ => return,
            };

            state.block_applier.current = BlockApplierApplyState::PrepareDataPending {
                time: action.time_as_nanos(),
                storage_req_id: content.storage_req_id,
                chain_id: chain_id.clone(),
                block_hash: block_hash.clone(),
            };
        }
        Action::BlockApplierApplyPrepareDataSuccess(content) => {
            let (time, chain_id) = match &state.block_applier.current {
                BlockApplierApplyState::PrepareDataPending { time, chain_id, .. } => {
                    (time, chain_id)
                }
                _ => return,
            };

            state.block_applier.current = BlockApplierApplyState::PrepareDataSuccess {
                time: action.time_as_nanos(),
                prepare_data_duration: action.time_as_nanos() - time,
                chain_id: chain_id.clone(),
                block: content.block.clone(),
                block_meta: content.block_meta.clone(),
                apply_block_req: content.apply_block_req.clone(),
            };
        }
        Action::BlockApplierApplyProtocolRunnerApplyPending(_) => {
            match &state.block_applier.current {
                BlockApplierApplyState::PrepareDataSuccess {
                    prepare_data_duration,
                    chain_id,
                    block,
                    block_meta,
                    apply_block_req,
                    ..
                } => {
                    state.block_applier.current =
                        BlockApplierApplyState::ProtocolRunnerApplyPending {
                            time: action.time_as_nanos(),
                            prepare_data_duration: prepare_data_duration.clone(),
                            chain_id: chain_id.clone(),
                            block: block.clone(),
                            block_meta: block_meta.clone(),
                            apply_block_req: apply_block_req.clone(),
                        };
                }
                _ => return,
            };
        }
        Action::BlockApplierApplyProtocolRunnerApplySuccess(content) => {
            match &state.block_applier.current {
                BlockApplierApplyState::ProtocolRunnerApplyPending {
                    time,
                    prepare_data_duration,
                    chain_id,
                    block,
                    block_meta,
                    ..
                } => {
                    state.block_applier.current =
                        BlockApplierApplyState::ProtocolRunnerApplySuccess {
                            time: action.time_as_nanos(),
                            prepare_data_duration: *prepare_data_duration,
                            protocol_runner_apply_duration: action.time_as_nanos() - time,
                            chain_id: chain_id.clone(),
                            block: block.clone(),
                            block_meta: block_meta.clone(),
                            apply_result: content.apply_result.clone(),
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
                    chain_id,
                    block,
                    block_meta,
                    apply_result,
                    ..
                } => {
                    state.block_applier.current = BlockApplierApplyState::StoreApplyResultPending {
                        time: action.time_as_nanos(),
                        prepare_data_duration: *prepare_data_duration,
                        protocol_runner_apply_duration: *protocol_runner_apply_duration,
                        storage_req_id: content.storage_req_id.clone(),
                        chain_id: chain_id.clone(),
                        block: block.clone(),
                        block_meta: block_meta.clone(),
                        apply_result: apply_result.clone(),
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
                    chain_id,
                    block,
                    apply_result,
                    ..
                } => {
                    state.block_applier.current = BlockApplierApplyState::StoreApplyResultSuccess {
                        time: action.time_as_nanos(),
                        prepare_data_duration: *prepare_data_duration,
                        protocol_runner_apply_duration: *protocol_runner_apply_duration,
                        store_apply_result_duration: action.time_as_nanos() - time,
                        chain_id: chain_id.clone(),
                        block: block.clone(),
                        block_additional_data: content.block_additional_data.clone(),
                        apply_result: apply_result.clone(),
                    };
                }
                _ => return,
            };
        }
        Action::BlockApplierApplySuccess(_) => {
            match &state.block_applier.current {
                BlockApplierApplyState::StoreApplyResultSuccess {
                    time,
                    prepare_data_duration,
                    protocol_runner_apply_duration,
                    store_apply_result_duration,
                    chain_id,
                    block,
                    block_additional_data,
                    apply_result,
                } => {
                    state.block_applier.last_applied = block.hash.clone().into();
                    state.block_applier.current = BlockApplierApplyState::Success {
                        time: time.clone(),
                        prepare_data_duration: prepare_data_duration.clone(),
                        protocol_runner_apply_duration: protocol_runner_apply_duration.clone(),
                        store_apply_result_duration: store_apply_result_duration.clone(),
                        chain_id: chain_id.clone(),
                        block: block.clone(),
                        block_additional_data: block_additional_data.clone(),
                        apply_result: apply_result.clone(),
                    };
                }
                _ => return,
            };
        }
        _ => {}
    }
}
