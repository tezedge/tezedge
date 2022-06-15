// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::block_applier::BlockApplierApplyState;
use crate::{Action, ActionWithMeta, State};

use super::CurrentHeadState;

pub fn current_head_reducer(state: &mut State, action: &ActionWithMeta) {
    match &action.action {
        Action::CurrentHeadRehydrateInit(_) => {
            state.current_head = CurrentHeadState::RehydrateInit {
                time: action.time_as_nanos(),
            };
        }
        Action::CurrentHeadRehydratePending(content) => {
            state.current_head = CurrentHeadState::RehydratePending {
                time: action.time_as_nanos(),
                storage_req_id: content.storage_req_id,
            };
        }
        Action::CurrentHeadRehydrateError(content) => {
            state.current_head = CurrentHeadState::RehydrateError {
                time: action.time_as_nanos(),
                error: content.error.clone(),
            };
        }
        Action::CurrentHeadRehydrateSuccess(content) => {
            state.current_head = CurrentHeadState::RehydrateSuccess {
                time: action.time_as_nanos(),
                head: content.head.clone(),
                head_pred: content.head_pred.clone(),
                block_metadata_hash: content.block_metadata_hash.clone(),
                ops_metadata_hash: content.ops_metadata_hash.clone(),
                pred_block_metadata_hash: content.pred_block_metadata_hash.clone(),
                pred_ops_metadata_hash: content.pred_ops_metadata_hash.clone(),
                cycle: content.cycle.clone(),
                operations: content.operations.clone(),
                constants: content.constants.clone(),
            };
        }
        Action::CurrentHeadRehydrated(_) => match &mut state.current_head {
            CurrentHeadState::RehydrateSuccess {
                head,
                head_pred,
                block_metadata_hash,
                ops_metadata_hash,
                pred_block_metadata_hash,
                pred_ops_metadata_hash,
                cycle,
                operations,
                constants,
                ..
            } => {
                let mut new_head = CurrentHeadState::rehydrated(head.clone(), head_pred.clone());
                new_head
                    .set_block_metadata_hash(block_metadata_hash.clone())
                    .set_ops_metadata_hash(ops_metadata_hash.clone())
                    .set_pred_block_metadata_hash(pred_block_metadata_hash.clone())
                    .set_pred_ops_metadata_hash(pred_ops_metadata_hash.clone())
                    .set_cycle(cycle.clone())
                    .set_operations(std::mem::take(operations))
                    .set_constants(constants.clone());
                state.current_head = new_head;
            }
            _ => {}
        },
        Action::BlockApplierApplySuccess(_) => {
            let block = match &state.block_applier.current {
                BlockApplierApplyState::Success { block, .. } => &**block,
                _ => return,
            };
            state.current_head.add_applied_block(block, None);
        }
        Action::CurrentHeadUpdate(content) => {
            state
                .current_head
                .add_applied_block(&content.new_head, Some(content.max_operations_ttl));
            let applied_blocks = match &mut state.current_head {
                CurrentHeadState::Rehydrated { applied_blocks, .. } => {
                    std::mem::take(applied_blocks)
                }
                _ => return,
            };
            let head = content.new_head.clone();
            let head_pred = state
                .current_head
                .get()
                .filter(|pred| &pred.hash == head.header.predecessor())
                .or_else(|| state.current_head.get_pred())
                .filter(|pred| &pred.hash == head.header.predecessor())
                .or_else(|| {
                    applied_blocks
                        .get(head.header.predecessor())
                        .map(|(b, _)| b)
                })
                .cloned();
            let payload_round = head.header.payload_round();
            let constants = content
                .new_constants
                .as_ref()
                .or(state.current_head.constants())
                .cloned();
            state.current_head =
                CurrentHeadState::Rehydrated {
                    head,
                    head_pred,
                    payload_hash: content.payload_hash.clone(),
                    payload_round,
                    block_metadata_hash: content.block_metadata_hash.clone(),
                    ops_metadata_hash: content.ops_metadata_hash.clone(),
                    pred_block_metadata_hash: content.pred_block_metadata_hash.clone(),
                    pred_ops_metadata_hash: content.pred_ops_metadata_hash.clone(),
                    cycle: content.cycle.clone(),
                    operations: content.operations.clone(),
                    constants,
                    applied_blocks,
                };
        }
        _ => {}
    }
}
