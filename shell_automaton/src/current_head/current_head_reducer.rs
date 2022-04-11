// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

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
            };
        }
        Action::CurrentHeadRehydrated(_) => {
            let (head, head_pred, block_metadata_hash, ops_metadata_hash) =
                match &state.current_head {
                    CurrentHeadState::RehydrateSuccess {
                        head,
                        head_pred,
                        block_metadata_hash,
                        ops_metadata_hash,
                        ..
                    } => (
                        head.clone(),
                        head_pred.clone(),
                        block_metadata_hash.clone(),
                        ops_metadata_hash.clone(),
                    ),
                    _ => return,
                };

            state.current_head = CurrentHeadState::rehydrated(head, head_pred);
            state
                .current_head
                .set_block_metadata_hash(block_metadata_hash)
                .set_ops_metadata_hash(ops_metadata_hash);
        }
        Action::CurrentHeadUpdate(content) => {
            let mut applied_blocks = match &mut state.current_head {
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
                .or_else(|| applied_blocks.get(head.header.predecessor()))
                .cloned();

            applied_blocks.insert(head.hash.clone(), head.clone());
            applied_blocks.retain(|_, b| b.header.level() + 1 >= head.header.level());

            state.current_head = CurrentHeadState::Rehydrated {
                head,
                head_pred,
                payload_hash: content.payload_hash.clone(),
                block_metadata_hash: content.block_metadata_hash.clone(),
                ops_metadata_hash: content.ops_metadata_hash.clone(),
                applied_blocks,
            };
        }
        _ => {}
    }
}
