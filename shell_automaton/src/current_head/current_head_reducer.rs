// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::btree_map::Entry as BTreeMapEntry;

use tezos_messages::protocol::SupportedProtocol;

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
                cemented_live_blocks: content.cemented_live_blocks.clone(),
                proto_cache: content.proto_cache.clone(),
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
                cemented_live_blocks,
                proto_cache,
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
                    .set_constants(constants.clone())
                    .set_cemented_live_blocks(cemented_live_blocks.clone())
                    .set_proto_cache(std::mem::take(proto_cache));
                state.current_head = new_head;
            }
            _ => {}
        },
        Action::BlockApplierApplySuccess(_) => {
            let block = match &state.block_applier.current {
                BlockApplierApplyState::Success { block, .. } => &**block,
                _ => return,
            };
            state.current_head.add_applied_block(block);
        }
        Action::CurrentHeadUpdate(content) => {
            state.current_head.add_applied_block(&content.new_head);
            let (applied_blocks, mut proto_cache) = match &mut state.current_head {
                CurrentHeadState::Rehydrated {
                    applied_blocks,
                    proto_cache,
                    ..
                } => (std::mem::take(applied_blocks), std::mem::take(proto_cache)),
                _ => return,
            };
            let proto = content.new_head.header.proto();
            if let BTreeMapEntry::Vacant(e) = proto_cache.entry(proto) {
                match SupportedProtocol::try_from(&content.next_protocol) {
                    Ok(protocol) => {
                        e.insert(protocol);
                    }
                    Err(err) => {
                        slog::error!(&state.log, "Detected unknown protocol while updating current head";
                            "protocol_hash" => content.next_protocol.to_base58_check(),
                            "proto" => proto,
                            "new_current_head" => format!("{:?}", content.new_head),
                            "err" => format!("{:?}", err));
                    }
                }
            }

            let head = content.new_head.clone();
            let head_pred = state
                .current_head
                .get()
                .filter(|pred| &pred.hash == head.header.predecessor())
                .or_else(|| state.current_head.get_pred())
                .filter(|pred| &pred.hash == head.header.predecessor())
                .or_else(|| applied_blocks.get(head.header.predecessor()))
                .cloned();
            let payload_round = head.header.payload_round();
            let constants = content
                .new_constants
                .as_ref()
                .or(state.current_head.constants())
                .cloned();
            if let Some(pred) = head_pred.as_ref() {
                let level = pred.header.level();
                if level > 0 {
                    let hash = pred.header.predecessor().clone();
                    state.current_head.add_cemented_live_block(hash, level - 1);
                }
            }
            let cemented_live_blocks = match &mut state.current_head {
                CurrentHeadState::Rehydrated {
                    cemented_live_blocks,
                    ..
                } => std::mem::take(cemented_live_blocks),
                _ => return,
            };

            state.current_head = CurrentHeadState::Rehydrated {
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
                cemented_live_blocks,
                proto_cache,
            };
        }
        _ => {}
    }
}
