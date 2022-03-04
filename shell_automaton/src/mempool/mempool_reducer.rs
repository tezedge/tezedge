// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;
use std::mem;
use std::net::SocketAddr;

use crypto::hash::OperationHash;
use tezos_messages::p2p::binary_message::MessageHash;
use tezos_messages::p2p::encoding::peer::PeerMessage;

use crate::block_applier::BlockApplierApplyState;
use crate::mempool::OperationKind;
use crate::peers::remove::PeersRemoveAction;
use crate::protocol::ProtocolAction;
use crate::{Action, ActionWithMeta, State};

use super::{
    mempool_actions::*,
    mempool_state::{HeadState, MempoolOperation, OperationStream},
};
use super::{
    OperationNodeCurrentHeadStats, OperationState, OperationStats, OperationValidationResult,
};
use crate::prechecker::prechecker_actions::{
    PrecheckerPrecheckOperationRequestAction, PrecheckerPrecheckOperationResponse,
    PrecheckerPrecheckOperationResponseAction, PrecheckerPrevalidate,
};

pub fn mempool_reducer(state: &mut State, action: &ActionWithMeta) {
    if state.config.disable_mempool {
        return;
    }
    let config = &state.config;
    let mempool_state = &mut state.mempool;

    match &action.action {
        Action::Protocol(act) => match act {
            ProtocolAction::PrevalidatorReady(prevalidator) => {
                mempool_state.prevalidator = Some(prevalidator.clone());
                if let Some(local_head_state) = mempool_state.local_head_state.as_mut() {
                    local_head_state.prevalidator_ready = true;
                }
            }
            ProtocolAction::OperationValidated(result) => {
                let current_head_level = mempool_state
                    .local_head_state
                    .as_ref()
                    .map(|v| v.header.level());
                mempool_state.prevalidator = Some(result.prevalidator.clone());
                for v in &result.result.applied {
                    if let Some(op) = mempool_state.pending_operations.remove(&v.hash) {
                        mempool_state
                            .validated_operations
                            .ops
                            .insert(v.hash.clone().into(), op);
                        mempool_state.validated_operations.applied.push(v.clone());
                        for (_, peer) in &mut mempool_state.peer_state {
                            if !peer.seen_operations.contains(&v.hash) {
                                peer.known_valid_to_send.push(v.hash.clone());
                            }
                        }
                        mempool_state
                            .operation_stats
                            .entry(v.hash.clone().into())
                            .or_insert_with(|| OperationStats::new())
                            .validation_finished(
                                action.time_as_nanos(),
                                Some(result.validate_operation_started_at),
                                Some(result.validate_operation_ended_at),
                                current_head_level,
                                OperationValidationResult::Applied,
                            );
                    }
                    if let Some(rpc_id) = mempool_state.injecting_rpc_ids.remove(&v.hash) {
                        mempool_state.injected_rpc_ids.push(rpc_id);
                    }
                    if let Some(operation_state) = mempool_state.operations_state.get_mut(&v.hash) {
                        if let MempoolOperation {
                            state: OperationState::Decoded,
                            ..
                        } = operation_state
                        {
                            *operation_state =
                                operation_state.next_state(OperationState::Applied, action);
                        }
                    }
                }
                for v in &result.result.refused {
                    if let Some(op) = mempool_state.pending_operations.remove(&v.hash) {
                        mempool_state
                            .validated_operations
                            .refused_ops
                            .insert(v.hash.clone().into(), op);
                        mempool_state.validated_operations.refused.push(v.clone());
                        mempool_state
                            .operation_stats
                            .entry(v.hash.clone().into())
                            .or_insert_with(|| OperationStats::new())
                            .validation_finished(
                                action.time_as_nanos(),
                                Some(result.validate_operation_started_at),
                                Some(result.validate_operation_ended_at),
                                current_head_level,
                                OperationValidationResult::Refused,
                            );
                    }
                    if let Some(rpc_id) = mempool_state.injecting_rpc_ids.remove(&v.hash) {
                        mempool_state.injected_rpc_ids.push(rpc_id);
                    }
                    if let Some(operation_state) = mempool_state.operations_state.get_mut(&v.hash) {
                        if let MempoolOperation {
                            state: OperationState::Decoded,
                            ..
                        } = operation_state
                        {
                            *operation_state =
                                operation_state.next_state(OperationState::Refused, action);
                        }
                    }
                }
                for v in &result.result.branch_refused {
                    if let Some(op) = mempool_state.pending_operations.remove(&v.hash) {
                        mempool_state
                            .validated_operations
                            .ops
                            .insert(v.hash.clone().into(), op);
                        mempool_state
                            .validated_operations
                            .branch_refused
                            .push(v.clone());
                        mempool_state
                            .operation_stats
                            .entry(v.hash.clone().into())
                            .or_insert_with(|| OperationStats::new())
                            .validation_finished(
                                action.time_as_nanos(),
                                Some(result.validate_operation_started_at),
                                Some(result.validate_operation_ended_at),
                                current_head_level,
                                OperationValidationResult::BranchRefused,
                            );
                    }
                    if let Some(rpc_id) = mempool_state.injecting_rpc_ids.remove(&v.hash) {
                        mempool_state.injected_rpc_ids.push(rpc_id);
                    }
                    if let Some(operation_state) = mempool_state.operations_state.get_mut(&v.hash) {
                        if let MempoolOperation {
                            state: OperationState::Decoded,
                            ..
                        } = operation_state
                        {
                            *operation_state =
                                operation_state.next_state(OperationState::BranchRefused, action);
                        }
                    }
                }
                for v in &result.result.branch_delayed {
                    if let Some(op) = mempool_state.pending_operations.remove(&v.hash) {
                        mempool_state
                            .validated_operations
                            .ops
                            .insert(v.hash.clone().into(), op);
                        mempool_state
                            .validated_operations
                            .branch_delayed
                            .push(v.clone());
                        if v.is_endorsement.unwrap_or(false) {
                            for (_, peer) in &mut mempool_state.peer_state {
                                if !peer.seen_operations.contains(&v.hash) {
                                    peer.known_valid_to_send.push(v.hash.clone());
                                }
                            }
                        }
                        mempool_state
                            .operation_stats
                            .entry(v.hash.clone().into())
                            .or_insert_with(|| OperationStats::new())
                            .validation_finished(
                                action.time_as_nanos(),
                                Some(result.validate_operation_started_at),
                                Some(result.validate_operation_ended_at),
                                current_head_level,
                                OperationValidationResult::BranchDelayed,
                            );
                    }
                    if let Some(rpc_id) = mempool_state.injecting_rpc_ids.remove(&v.hash) {
                        mempool_state.injected_rpc_ids.push(rpc_id);
                    }
                    if let Some(operation_state) = mempool_state.operations_state.get_mut(&v.hash) {
                        if let MempoolOperation {
                            state: OperationState::Decoded,
                            ..
                        } = operation_state
                        {
                            *operation_state =
                                operation_state.next_state(OperationState::BranchDelayed, action);
                        }
                    }
                }
            }
            _ => {}
        },
        Action::BlockApplierApplySuccess(_) => {
            // TODO: get from protocol
            const TTL: i32 = 120;

            let (chain_id, block, apply_result, retry) = match &state.block_applier.current {
                BlockApplierApplyState::Success {
                    chain_id,
                    block,
                    apply_result,
                    retry,
                    ..
                } => (chain_id, block, apply_result, retry),
                _ => return,
            };

            if config.chain_id.ne(chain_id) {
                return;
            }

            if let Some(local_head_state) = &mempool_state.local_head_state {
                let local_header = &local_head_state.header;
                let new_header = &block.header;
                if local_header.level() == new_header.level()
                    && local_header.predecessor() == new_header.predecessor()
                {
                    slog::info!(
                        &state.log,
                        "Block `{new_block}` applied on the same level, ignoring it",
                        new_block = block.hash.to_base58_check();
                        "head" => slog::FnValue(|_| local_head_state.hash.to_base58_check())
                    );
                    return;
                }
            }

            if retry.is_some() {
                if config.disable_apply_retry {
                    slog::info!(
                        &state.log,
                        "Block `{new_block}` applied after retry, not using it as current head",
                        new_block = block.hash.to_base58_check();
                    );
                    return;
                } else {
                    slog::info!(
                        &state.log,
                        "Block `{new_block}` applied after retry",
                        new_block = block.hash.to_base58_check();
                    );
                }
            }

            let old_head_state = mempool_state.local_head_state.clone();
            mempool_state.branch_changed = old_head_state
                .as_ref()
                .map(|old_head| old_head.hash.ne(&block.header.predecessor()))
                .unwrap_or(false);
            mempool_state.local_head_state = Some(HeadState {
                header: (*block.header).clone(),
                hash: block.hash.clone(),
                ops_removed: false,
                prevalidator_ready: false,
                metadata_hash: apply_result.block_metadata_hash.clone(),
                ops_metadata_hash: apply_result.ops_metadata_hash.clone(),
            });
            mempool_state.prevalidator = None;

            if state.is_bootstrapped() {
                state.mempool.running_since = Some(());
            }
            let mempool_state = &mut state.mempool;

            // update last 120 predecessor blocks map.
            let last_predecessor_blocks = &mut mempool_state.last_predecessor_blocks;
            last_predecessor_blocks
                .insert(block.header.predecessor().clone(), block.header.level() - 1);
            if last_predecessor_blocks.len() as i32 > TTL {
                if let Some((oldest, _)) = last_predecessor_blocks
                    .iter()
                    .min_by(|(_, l0), (_, l1)| l0.cmp(l1))
                {
                    let oldest = oldest.clone();
                    last_predecessor_blocks.remove(&oldest);
                }
            }

            // for (_, op) in &mempool_state.pending_operations {
            //     mempool_state.wait_prevalidator_operations.push(op.clone());
            // }

            let level = block.header.level().saturating_sub(TTL);

            // `drain_filter` is unstable for now
            for (_, ops) in mempool_state.level_to_operation.range(..level) {
                for op in ops {
                    mempool_state.pending_operations.remove(op);
                    mempool_state.validated_operations.ops.remove(op);
                    mempool_state.validated_operations.refused_ops.remove(op);
                    mempool_state
                        .validated_operations
                        .applied
                        .retain(|v| v.hash.ne(op));
                    mempool_state
                        .validated_operations
                        .refused
                        .retain(|v| v.hash.ne(op));
                    mempool_state
                        .validated_operations
                        .branch_delayed
                        .retain(|v| v.hash.ne(op));
                    mempool_state
                        .validated_operations
                        .branch_refused
                        .retain(|v| v.hash.ne(op));
                    for (_, peer_state) in &mut mempool_state.peer_state {
                        peer_state.seen_operations.remove(op);
                    }

                    // remove operation from stats
                    mempool_state.operation_stats.remove(op);
                }
            }
            mempool_state.level_to_operation.retain(|x, _| *x >= level);

            let start_level = old_head_state
                .as_ref()
                .map(|v| v.header.level())
                .unwrap_or(0);
            let end_level = block.header.level();

            if start_level >= end_level {
                return;
            }

            // Remove old endorsement operations.
            let range = start_level..end_level;
            for (_, ops) in mempool_state.level_to_operation.range(range) {
                for op in ops {
                    let is_endorsement = mempool_state
                        .validated_operations
                        .ops
                        .get(op)
                        .or_else(|| mempool_state.pending_operations.get(op))
                        .or_else(|| mempool_state.validated_operations.refused_ops.get(op))
                        .map(|op| OperationKind::from_operation_content_raw(&op.data()))
                        .filter(|op_kind| op_kind.is_endorsement())
                        .is_some();

                    if !is_endorsement {
                        continue;
                    }

                    mempool_state.pending_operations.remove(op);
                    mempool_state.validated_operations.ops.remove(op);
                    mempool_state.validated_operations.refused_ops.remove(op);
                    mempool_state
                        .validated_operations
                        .applied
                        .retain(|v| v.hash.ne(op));
                    mempool_state
                        .validated_operations
                        .refused
                        .retain(|v| v.hash.ne(op));
                    mempool_state
                        .validated_operations
                        .branch_delayed
                        .retain(|v| v.hash.ne(op));
                    mempool_state
                        .validated_operations
                        .branch_refused
                        .retain(|v| v.hash.ne(op));
                    for (_, peer_state) in &mut mempool_state.peer_state {
                        peer_state.seen_operations.remove(op);
                    }
                }
            }
        }
        Action::MempoolRecvDone(MempoolRecvDoneAction {
            address,
            block_hash,
            message,
            level,
            timestamp,
            proto: _,
        }) => {
            mempool_state.first_current_head = false;
            if let Some(local_head_state) = &mempool_state.local_head_state {
                if *level == local_head_state.header.level() + 1
                    && mempool_state
                        .latest_current_head
                        .as_ref()
                        .map_or(true, |hash| hash != block_hash)
                {
                    // new current_head
                    // remove older statuses
                    if let Some((l, _)) = mempool_state.old_operations_state.back() {
                        if *l < level - 20 {
                            // TODO how much?
                            mempool_state.old_operations_state.pop_back();
                        }
                    }
                    // insert previously current statuses
                    let old_state =
                        mem::replace(&mut mempool_state.operations_state, BTreeMap::new());
                    if let Some(local_head_state) = &mempool_state.local_head_state {
                        mempool_state
                            .old_operations_state
                            .push_front((local_head_state.header.level(), old_state));
                    }
                    mempool_state.latest_current_head = Some(block_hash.clone());
                    mempool_state.first_current_head = true;
                    mempool_state.first_current_head_time = action.time_as_nanos();
                }
            }

            let pending = message.pending().iter().cloned();
            let known_valid = message.known_valid().iter().cloned();

            let peer = mempool_state.peer_state.entry(*address).or_default();
            let ops = mempool_state.level_to_operation.entry(*level).or_default();

            for hash in pending.chain(known_valid) {
                let known = mempool_state.pending_operations.contains_key(&hash)
                    || mempool_state.validated_operations.ops.contains_key(&hash);

                if !known {
                    ops.push(hash.clone());
                    if !mempool_state.pending_full_content.contains(&hash) {
                        peer.requesting_full_content.insert(hash.clone());
                    }

                    mempool_state.operations_state.insert(
                        hash.clone().into(),
                        MempoolOperation::received(
                            *timestamp,
                            mempool_state.first_current_head_time,
                            action,
                        ),
                    );
                }
                // of course peer knows about it, because he sent us it
                peer.seen_operations.insert(hash);
            }
        }
        Action::MempoolMarkOperationsAsPending(MempoolMarkOperationsAsPendingAction {
            address,
        }) => {
            let peer = mempool_state.peer_state.entry(*address).or_default();
            mempool_state
                .pending_full_content
                .extend(peer.requesting_full_content.drain());
        }
        Action::MempoolOperationRecvDone(MempoolOperationRecvDoneAction { operation, .. }) => {
            let operation_hash = match operation.message_typed_hash() {
                Ok(v) => v,
                Err(err) => {
                    // TODO(vlad): peer send bad operation, should log the error,
                    // maybe should disconnect the peer
                    let _ = err;
                    return;
                }
            };
            if let Some(operation_state) = mempool_state.operations_state.get_mut(&operation_hash) {
                if let MempoolOperation {
                    state: OperationState::ReceivedHash,
                    ..
                } = operation_state
                {
                    *operation_state =
                        operation_state.next_state(OperationState::ReceivedContents, action);
                }
            }

            if !mempool_state.pending_full_content.remove(&operation_hash) {
                // TODO(vlad): received operation, but we did not requested it, what should we do?
            }

            // ignore endorsement operation if its for the past block.
            if mempool_state.is_old_endorsement(operation) {
                if let Some(level) = mempool_state
                    .last_predecessor_blocks
                    .get(operation.branch())
                {
                    if let Some(ops) = mempool_state.level_to_operation.get_mut(level) {
                        ops.retain(|op| op.ne(&operation_hash));
                    }
                    return;
                }
            }

            mempool_state
                .pending_operations
                .insert(operation_hash.into(), operation.clone());
        }
        Action::MempoolOperationInject(MempoolOperationInjectAction {
            operation,
            operation_hash,
            rpc_id,
            injected_timestamp,
        }) => {
            let level = mempool_state
                .local_head_state
                .as_ref()
                .map(|state| state.header.level())
                .unwrap_or(0);
            let ops = mempool_state.level_to_operation.entry(level).or_default();
            ops.push(operation_hash.clone());
            mempool_state
                .injecting_rpc_ids
                .insert(operation_hash.clone().into(), rpc_id.clone());
            mempool_state
                .pending_operations
                .insert(operation_hash.clone().into(), operation.clone());
            if let Some(local_head_state) = mempool_state.local_head_state.as_ref() {
                mempool_state.operations_state.insert(
                    operation_hash.clone().into(),
                    MempoolOperation::injected(
                        local_head_state.header.timestamp(),
                        mempool_state.first_current_head_time,
                        action,
                    ),
                );
            }

            let (block_level, block_timestamp) = match &mempool_state.local_head_state {
                Some(local_head_state) => (
                    local_head_state.header.level(),
                    local_head_state.header.timestamp(),
                ),
                _ => return,
            };
            let pkh = match state.config.identity.public_key.public_key_hash() {
                Ok(v) => v,
                Err(_) => return,
            };
            mempool_state
                .operation_stats
                .entry(operation_hash.clone().into())
                .or_insert(OperationStats::new())
                .received_via_rpc(
                    &pkh,
                    OperationNodeCurrentHeadStats {
                        time: action.time_as_nanos(),
                        block_level,
                        block_timestamp,
                    },
                    operation.data(),
                    injected_timestamp,
                );
        }
        Action::MempoolRegisterOperationsStream(act) => {
            mempool_state.operation_streams.push(OperationStream {
                rpc_id: act.rpc_id,
                applied: act.applied,
                refused: act.refused,
                branch_delayed: act.branch_delayed,
                branch_refused: act.branch_refused,
            });
        }
        Action::MempoolUnregisterOperationsStreams(MempoolUnregisterOperationsStreamsAction {}) => {
            mempool_state.operation_streams.clear();
        }
        Action::MempoolValidateWaitPrevalidator(MempoolValidateWaitPrevalidatorAction {
            operation,
        }) => {
            mempool_state
                .wait_prevalidator_operations
                .push(operation.clone());
        }
        Action::MempoolCleanupWaitPrevalidator(MempoolCleanupWaitPrevalidatorAction {}) => {
            mempool_state.wait_prevalidator_operations.clear();
        }
        Action::MempoolRpcRespond(MempoolRpcRespondAction {}) => {
            mempool_state.injected_rpc_ids.clear();
        }
        Action::MempoolSend(MempoolSendAction { address, .. }) => {
            mempool_state.peer_state.entry(*address).or_default();
        }
        Action::PeersRemove(PeersRemoveAction { address }) => {
            mempool_state.peer_state.remove(address);
        }
        Action::PrecheckerPrecheckOperationResponse(
            PrecheckerPrecheckOperationResponseAction { response },
        ) => {
            match response {
                PrecheckerPrecheckOperationResponse::Applied(applied) => {
                    let hash = &applied.hash;
                    if let Some(op) = mempool_state.pending_operations.remove(hash) {
                        mempool_state
                            .validated_operations
                            .ops
                            .insert(hash.clone().into(), op);
                        mempool_state
                            .validated_operations
                            .applied
                            .push(applied.as_applied());
                    }
                    if let Some(rpc_id) = mempool_state.injecting_rpc_ids.remove(hash) {
                        mempool_state.injected_rpc_ids.push(rpc_id);
                    }
                    if let Some(operation_state) = mempool_state.operations_state.get_mut(hash) {
                        if let MempoolOperation {
                            state: OperationState::Decoded,
                            ..
                        } = operation_state
                        {
                            *operation_state =
                                operation_state.next_state(OperationState::Prechecked, action);
                        }
                    }
                    let current_head_level = mempool_state
                        .local_head_state
                        .as_ref()
                        .map(|v| v.header.level());
                    mempool_state
                        .operation_stats
                        .entry(hash.clone().into())
                        .or_insert_with(|| OperationStats::new())
                        .validation_finished(
                            action.time_as_nanos(),
                            None,
                            None,
                            current_head_level,
                            OperationValidationResult::Prechecked,
                        );
                }
                PrecheckerPrecheckOperationResponse::Refused(errored) => {
                    let hash = &errored.hash;
                    if let Some(op) = mempool_state.pending_operations.remove(&errored.hash) {
                        mempool_state
                            .validated_operations
                            .refused_ops
                            .insert(errored.hash.clone().into(), op);
                        mempool_state
                            .validated_operations
                            .refused
                            .push(errored.as_errored());
                    }
                    if let Some(rpc_id) = mempool_state.injecting_rpc_ids.remove(&errored.hash) {
                        mempool_state.injected_rpc_ids.push(rpc_id);
                    }
                    if let Some(operation_state) = mempool_state.operations_state.get_mut(hash) {
                        if let MempoolOperation {
                            state: OperationState::Decoded,
                            ..
                        } = operation_state
                        {
                            let next =
                                operation_state.next_state(OperationState::PrecheckRefused, action);
                            *operation_state = next;
                        }
                    }
                    let current_head_level = mempool_state
                        .local_head_state
                        .as_ref()
                        .map(|v| v.header.level());
                    mempool_state
                        .operation_stats
                        .entry(hash.clone().into())
                        .or_insert_with(|| OperationStats::new())
                        .validation_finished(
                            action.time_as_nanos(),
                            None,
                            None,
                            current_head_level,
                            OperationValidationResult::PrecheckRefused,
                        );
                }
                PrecheckerPrecheckOperationResponse::Prevalidate(PrecheckerPrevalidate {
                    hash,
                }) => {
                    let current_head_level = mempool_state
                        .local_head_state
                        .as_ref()
                        .map(|v| v.header.level());
                    mempool_state
                        .operation_stats
                        .entry(hash.clone().into())
                        .or_insert_with(|| OperationStats::new())
                        .validation_finished(
                            action.time_as_nanos(),
                            None,
                            None,
                            current_head_level,
                            OperationValidationResult::Prevalidate,
                        );
                }
                PrecheckerPrecheckOperationResponse::Error(_) => {
                    // TODO
                }
            }
        }
        Action::MempoolBroadcastDone(MempoolBroadcastDoneAction {
            address,
            known_valid,
            pending,
            cleanup_known_valid,
        }) => {
            let peer = mempool_state.peer_state.entry(*address).or_default();

            peer.seen_operations.extend(known_valid.iter().cloned());
            peer.seen_operations.extend(pending.iter().cloned());
            if *cleanup_known_valid {
                peer.known_valid_to_send.clear();
            }
            for hash in known_valid {
                if let Some(operation_state) = mempool_state.operations_state.get_mut(hash) {
                    match operation_state {
                        MempoolOperation {
                            state: OperationState::Prechecked,
                            ..
                        }
                        | MempoolOperation {
                            state: OperationState::Applied,
                            ..
                        } => *operation_state = operation_state.broadcast(action),
                        _ => (),
                    }
                }
            }
        }
        Action::MempoolRemoveAppliedOperations(MempoolRemoveAppliedOperationsAction {
            operation_hashes,
        }) => {
            mempool_state
                .validated_operations
                .applied
                .retain(|v| !operation_hashes.contains(&v.hash));
            mempool_state
                .validated_operations
                .branch_delayed
                .retain(|v| !operation_hashes.contains(&v.hash));
            for op in operation_hashes {
                mempool_state.validated_operations.ops.remove(op);
            }
            if let Some(state) = &mut mempool_state.local_head_state {
                state.ops_removed = true;
            }
        }
        Action::MempoolFlush(MempoolFlushAction {}) => {
            if mempool_state.branch_changed {
                // remove all `branch_refused` results, put them into `pending_operations`
                // to validate again with new prevalidator
                for v in mem::take(&mut mempool_state.validated_operations.branch_refused) {
                    if let Some(op) = mempool_state.validated_operations.ops.remove(&v.hash) {
                        mempool_state
                            .pending_operations
                            .insert(v.hash.into(), op.clone());
                        mempool_state.wait_prevalidator_operations.push(op);
                    }
                }
            } else {
                // remove all remaining `applied` results and all `branch_delayed` results,
                // put them into `pending_operations` to validate again with new prevalidator
                for v in mem::take(&mut mempool_state.validated_operations.branch_delayed) {
                    if let Some(op) = mempool_state.validated_operations.ops.remove(&v.hash) {
                        mempool_state
                            .pending_operations
                            .insert(v.hash.into(), op.clone());
                        mempool_state.wait_prevalidator_operations.push(op);
                    }
                }
                for v in mem::take(&mut mempool_state.validated_operations.applied) {
                    if let Some(op) = mempool_state.validated_operations.ops.remove(&v.hash) {
                        mempool_state
                            .pending_operations
                            .insert(v.hash.into(), op.clone());
                        mempool_state.wait_prevalidator_operations.push(op);
                    }
                }
            }
        }
        Action::PrecheckerPrecheckOperationRequest(PrecheckerPrecheckOperationRequestAction {
            operation,
        })
        | Action::MempoolValidateStart(MempoolValidateStartAction { operation }) => {
            let op_hash = match operation.message_typed_hash() {
                Ok(v) => v,
                Err(_) => return,
            };
            let current_head_level = mempool_state
                .local_head_state
                .as_ref()
                .map(|v| v.header.level());
            mempool_state
                .operation_stats
                .entry(op_hash)
                .or_insert(OperationStats::new())
                .validation_started(action.time_as_nanos(), current_head_level);
        }
        Action::PeerMessageReadSuccess(content) => {
            if mempool_state.running_since.is_none() {
                return;
            }

            let peer = match state
                .peers
                .get(&content.address)
                .and_then(|peer| peer.status.as_handshaked())
            {
                Some(v) => v,
                None => return,
            };
            let peer_pkh = &peer.public_key_hash;
            let time = action.time_as_nanos();

            match content.message.message() {
                PeerMessage::CurrentHead(msg) => {
                    let block_header = msg.current_block_header();
                    let mempool = msg.current_mempool();
                    let op_hash_iter = mempool
                        .pending()
                        .iter()
                        .chain(mempool.known_valid())
                        .cloned();

                    for op_hash in op_hash_iter {
                        mempool_state
                            .operation_stats
                            .entry(op_hash.into())
                            .or_insert(OperationStats::new())
                            .received_in_current_head(
                                peer_pkh,
                                OperationNodeCurrentHeadStats {
                                    time,
                                    block_level: block_header.level(),
                                    block_timestamp: block_header.timestamp(),
                                },
                            );
                    }
                }
                PeerMessage::GetOperations(msg) => {
                    for op_hash in msg.get_operations().iter().cloned() {
                        mempool_state
                            .operation_stats
                            .entry(op_hash.into())
                            .or_insert(OperationStats::new())
                            .content_requested_remote(peer_pkh, time);
                    }
                }
                PeerMessage::Operation(msg) => {
                    let op_hash = match msg.operation().message_typed_hash() {
                        Ok(v) => v,
                        Err(_) => return,
                    };

                    mempool_state
                        .operation_stats
                        .entry(op_hash)
                        .or_insert(OperationStats::new())
                        .content_received(peer_pkh, time, msg.operation().data());
                }
                _ => return,
            };
        }
        Action::PeerMessageWriteInit(content) => {
            if mempool_state.running_since.is_none() {
                return;
            }

            match content.message.message() {
                PeerMessage::CurrentHead(_)
                | PeerMessage::GetOperations(_)
                | PeerMessage::Operation(_) => {}
                _ => return,
            }
            update_operation_sent_stats(state, content.address, action.time_as_nanos());
        }
        Action::PeerMessageWriteNext(content) => {
            if mempool_state.running_since.is_none() {
                return;
            }

            update_operation_sent_stats(state, content.address, action.time_as_nanos());
        }
        Action::MempoolOperationDecoded(MempoolOperationDecodedAction {
            operation,
            operation_decoded_contents,
        }) => {
            if let Some(operation_state) = mempool_state.operations_state.get_mut(operation) {
                if let MempoolOperation {
                    state: OperationState::ReceivedContents,
                    ..
                } = operation_state
                {
                    *operation_state =
                        operation_state.decoded(operation_decoded_contents.clone(), action);
                }
            }
        }
        _ => (),
    }
}

fn update_operation_sent_stats(state: &mut State, address: SocketAddr, time: u64) {
    let peer = match state.peers.get(&address) {
        Some(v) => match v.status.as_handshaked() {
            Some(v) => v,
            None => return,
        },
        None => return,
    };
    let msg = match peer.message_write.queue.front() {
        Some(v) => v.message(),
        None => return,
    };

    match msg {
        PeerMessage::CurrentHead(msg) => {
            let block_header = msg.current_block_header();
            let mempool = msg.current_mempool();
            let op_hash_iter = mempool
                .pending()
                .iter()
                .chain(mempool.known_valid())
                .cloned();
            let pkh = &peer.public_key_hash;

            for op_hash in op_hash_iter {
                state
                    .mempool
                    .operation_stats
                    .entry(op_hash.into())
                    .or_insert(OperationStats::new())
                    .sent_in_current_head(
                        pkh,
                        OperationNodeCurrentHeadStats {
                            time,
                            block_level: block_header.level(),
                            block_timestamp: block_header.timestamp(),
                        },
                    );
            }
        }
        PeerMessage::GetOperations(msg) => {
            for op_hash in msg.get_operations().iter().cloned() {
                state
                    .mempool
                    .operation_stats
                    .entry(op_hash.into())
                    .or_insert(OperationStats::new())
                    .content_requested(&peer.public_key_hash, time);
            }
        }
        PeerMessage::Operation(msg) => {
            let op_hash: OperationHash = match msg.operation().message_typed_hash() {
                Ok(v) => v,
                Err(_) => return,
            };

            state
                .mempool
                .operation_stats
                .entry(op_hash.into())
                .or_insert(OperationStats::new())
                .content_sent(&peer.public_key_hash, time);
        }
        _ => return,
    };
}
