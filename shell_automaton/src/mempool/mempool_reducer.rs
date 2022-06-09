// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::mem;
use std::net::SocketAddr;
use std::time::Instant;

use crypto::hash::OperationHash;
use tezos_api::ffi::{Errored, HasOperationHash, Validated};
use tezos_messages::p2p::binary_message::MessageHash;
use tezos_messages::p2p::encoding::operation::Operation;
use tezos_messages::p2p::encoding::peer::PeerMessage;

use crate::block_applier::BlockApplierApplyState;
use crate::peers::remove::PeersRemoveAction;
use crate::prechecker::{prechecking_enabled, OperationDecodedContents, PrecheckerResult};
use crate::{Action, ActionWithMeta, State};

use super::validator::{MempoolValidatorReclassifyOperationAction, MempoolValidatorValidateResult};
use super::{
    mempool_actions::*,
    mempool_state::{HeadState, MempoolOperation, OperationStream},
};
use super::{
    OperationKind, OperationNodeCurrentHeadStats, OperationState, OperationStats,
    OperationValidationResult,
};
use crate::prechecker::prechecker_actions::{
    PrecheckerOperationValidatedAction, PrecheckerProtocolNeededAction,
};

/// Number of levels to keep endorsements/preendorsements.
const OPERATION_STATUS_RETAIN_LEVELS: i32 = 120;

pub fn mempool_reducer(state: &mut State, action: &ActionWithMeta) {
    if state.config.disable_mempool {
        return;
    }
    let mempool_state = &mut state.mempool;

    match &action.action {
        Action::MempoolValidatorValidateSuccess(content) => {
            let current_head_level = state.current_head.get().map(|v| v.header.level());

            if let Some(rpc_id) = mempool_state.injecting_rpc_ids.remove(&content.op_hash) {
                mempool_state.injected_rpc_ids.push(rpc_id);
            }

            match &content.result {
                MempoolValidatorValidateResult::Applied(v) => {
                    if let Some(op) = mempool_state.pending_operations.remove(&v.hash) {
                        mempool_state
                            .validated_operations
                            .ops
                            .insert(v.hash.clone(), op);
                        mempool_state.validated_operations.applied.push(v.clone());
                        mempool_state
                            .operation_stats
                            .entry(v.hash.clone())
                            .or_insert_with(OperationStats::new)
                            .validation_finished(
                                action.time_as_nanos(),
                                Some(content.protocol_preapply_start),
                                Some(content.protocol_preapply_end),
                                current_head_level,
                                OperationValidationResult::Applied,
                            );
                        update_quorum_state_with_validated_operation(state, &v.hash);
                    }
                    let mempool_state = &mut state.mempool;
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
                MempoolValidatorValidateResult::Prechecked(v) => {
                    if let Some(op) = mempool_state.pending_operations.remove(&v.hash) {
                        mempool_state
                            .validated_operations
                            .ops
                            .insert(v.hash.clone(), op);
                        mempool_state.validated_operations.applied.push(v.clone());
                        mempool_state
                            .operation_stats
                            .entry(v.hash.clone())
                            .or_insert_with(OperationStats::new)
                            .validation_finished(
                                action.time_as_nanos(),
                                Some(content.protocol_preapply_start),
                                Some(content.protocol_preapply_end),
                                current_head_level,
                                OperationValidationResult::Prechecked,
                            );
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
                MempoolValidatorValidateResult::Refused(v) => {
                    if let Some(op) = mempool_state.pending_operations.remove(&v.hash) {
                        mempool_state
                            .validated_operations
                            .ops
                            .insert(v.hash.clone(), op);
                        mempool_state
                            .validated_operations
                            .refused
                            .push_back(v.clone());
                        mempool_state
                            .operation_stats
                            .entry(v.hash.clone())
                            .or_insert_with(OperationStats::new)
                            .validation_finished(
                                action.time_as_nanos(),
                                Some(content.protocol_preapply_start),
                                Some(content.protocol_preapply_end),
                                current_head_level,
                                OperationValidationResult::Refused,
                            );
                        mempool_state
                            .validated_operations
                            .enforce_max_refused_operations();
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
                MempoolValidatorValidateResult::BranchRefused(v) => {
                    if let Some(op) = mempool_state.pending_operations.remove(&v.hash) {
                        mempool_state
                            .validated_operations
                            .ops
                            .insert(v.hash.clone(), op);
                        mempool_state
                            .validated_operations
                            .branch_refused
                            .push_back(v.clone());
                        mempool_state
                            .operation_stats
                            .entry(v.hash.clone())
                            .or_insert_with(OperationStats::new)
                            .validation_finished(
                                action.time_as_nanos(),
                                Some(content.protocol_preapply_start),
                                Some(content.protocol_preapply_end),
                                current_head_level,
                                OperationValidationResult::BranchRefused,
                            );
                        mempool_state
                            .validated_operations
                            .enforce_max_refused_operations();
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
                MempoolValidatorValidateResult::BranchDelayed(v) => {
                    if let Some(op) = mempool_state.pending_operations.remove(&v.hash) {
                        mempool_state
                            .validated_operations
                            .ops
                            .insert(v.hash.clone(), op);
                        mempool_state
                            .validated_operations
                            .branch_delayed
                            .push_back(v.clone());
                        mempool_state
                            .operation_stats
                            .entry(v.hash.clone())
                            .or_insert_with(OperationStats::new)
                            .validation_finished(
                                action.time_as_nanos(),
                                Some(content.protocol_preapply_start),
                                Some(content.protocol_preapply_end),
                                current_head_level,
                                OperationValidationResult::BranchDelayed,
                            );
                        mempool_state
                            .validated_operations
                            .enforce_max_refused_operations();
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
                MempoolValidatorValidateResult::Outdated(v) => {
                    if let Some(op) = mempool_state.pending_operations.remove(&v.hash) {
                        mempool_state
                            .validated_operations
                            .ops
                            .insert(v.hash.clone(), op);
                        mempool_state
                            .validated_operations
                            .outdated
                            .push_back(v.clone());
                        mempool_state
                            .operation_stats
                            .entry(v.hash.clone())
                            .or_insert_with(OperationStats::new)
                            .validation_finished(
                                action.time_as_nanos(),
                                Some(content.protocol_preapply_start),
                                Some(content.protocol_preapply_end),
                                current_head_level,
                                OperationValidationResult::Outdated,
                            );
                        mempool_state
                            .validated_operations
                            .enforce_max_refused_operations();
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
                                operation_state.next_state(OperationState::Outdated, action);
                        }
                    }
                }
                MempoolValidatorValidateResult::Unparseable(operation_hash) => {
                    // Unparseable operations just get dropped and added to a registry
                    // so that we can avoid processing them if they show up again.
                    mempool_state.pending_operations.remove(operation_hash);
                    mempool_state.operations_state.remove(operation_hash);
                    mempool_state
                        .unparseable_operations
                        .insert(operation_hash.clone());
                }
            }
        }
        Action::MempoolValidatorReclassifyOperation(
            MempoolValidatorReclassifyOperationAction {
                op_hash,
                classification,
            },
        ) => {
            mempool_state
                .validated_operations
                .reclassify_manager_operation(op_hash, classification);
            mempool_state
                .validated_operations
                .enforce_max_refused_operations();
        }
        Action::BlockApplierApplySuccess(_) => {
            let block_operations = match &state.block_applier.current {
                BlockApplierApplyState::Success {
                    block_operations, ..
                } => block_operations,
                _ => return,
            };

            let t = Instant::now();
            // Remove operations that are included in applied block.
            let operations_in_block = block_operations
                .iter()
                .flatten()
                .filter_map(|op| op.message_typed_hash::<OperationHash>().ok())
                .collect::<BTreeSet<_>>();

            // Everytime the head changes, we forget about the known unparseables.
            // If the protocol changes thes may become parseable.
            mempool_state.unparseable_operations.clear();

            let applied = &mut mempool_state.validated_operations.applied;
            let branch_delayed = &mut mempool_state.validated_operations.branch_delayed;
            let ops = &mut mempool_state.validated_operations.ops;

            applied.retain(|v| !operations_in_block.contains(&v.hash));
            branch_delayed.retain(|v| !operations_in_block.contains(&v.hash));
            for op in operations_in_block {
                ops.remove(&op);
            }
            slog::debug!(state.log, "validated_operations: removed applied operations"; "time" => format!("{:?}", Instant::now() - t));
        }
        Action::PeerCurrentHeadUpdate(_) => {
            if state.is_bootstrapped() {
                state.mempool.running_since = Some(());
            }
        }
        Action::CurrentHeadRehydrated(_) | Action::CurrentHeadUpdate(_) => {
            let ttl = match state.current_head.constants() {
                Some(v) => v.max_operations_ttl,
                None => 120,
            };

            let block = match state.current_head.get() {
                Some(v) => v,
                None => return,
            };
            let old_head_state = mempool_state.local_head_state.clone();
            mempool_state.branch_changed = old_head_state
                .as_ref()
                .map(|old_head| old_head.hash.ne(block.header.predecessor()))
                .unwrap_or(false);
            mempool_state.local_head_state = Some(HeadState {
                header: (*block.header).clone(),
                hash: block.hash.clone(),
            });

            if state.is_bootstrapped() {
                state.mempool.running_since = Some(());
            }
            let mempool_state = &mut state.mempool;

            if !state.config.disable_endorsements_precheck {
                let t = Instant::now();
                if mempool_state.branch_changed {
                    // remove all `branch_refused` results, put them into `pending_operations`
                    // to validate again with new prevalidator
                    for v in drain_consensus_deq(
                        &mut mempool_state.validated_operations.branch_refused,
                        &mempool_state.validated_operations.ops,
                    ) {
                        if mempool_state
                            .validated_operations
                            .ops
                            .remove(&v.hash)
                            .is_some()
                        {
                            mempool_state.prechecking_delayed_operations.insert(v.hash);
                        }
                    }
                }
                // remove all remaining `applied` results and all `branch_delayed` results,
                // put them into `pending_operations` to validate again with prechecker
                for v in drain_consensus_deq(
                    &mut mempool_state.validated_operations.branch_delayed,
                    &mempool_state.validated_operations.ops,
                ) {
                    if mempool_state
                        .validated_operations
                        .ops
                        .remove(&v.hash)
                        .is_some()
                    {
                        mempool_state.prechecking_delayed_operations.insert(v.hash);
                    }
                }
                // remove all applied consensus operations
                for v in drain_consensus(
                    &mut mempool_state.validated_operations.applied,
                    &mempool_state.validated_operations.ops,
                ) {
                    mempool_state.validated_operations.ops.remove(&v.hash);
                }
                slog::debug!(state.log, "validated_operations: updated consensus operations"; "time" => format!("{:?}", Instant::now() - t));
            }

            let t = Instant::now();
            // update last 120 predecessor blocks map.
            let last_predecessor_blocks = &mut mempool_state.last_predecessor_blocks;
            last_predecessor_blocks
                .insert(block.header.predecessor().clone(), block.header.level() - 1);
            if last_predecessor_blocks.len() as i32 > ttl {
                if let Some((oldest, _)) = last_predecessor_blocks
                    .iter()
                    .min_by(|(_, l0), (_, l1)| l0.cmp(l1))
                {
                    let oldest = oldest.clone();
                    last_predecessor_blocks.remove(&oldest);
                }
            }

            let level = block.header.level().saturating_sub(ttl);

            // `drain_filter` is unstable for now
            for (_, ops) in mempool_state.level_to_operation.range(..level) {
                for op in ops {
                    mempool_state.pending_full_content.remove(op);
                    mempool_state.pending_operations.remove(op);
                    mempool_state.validated_operations.ops.remove(op);
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
                    mempool_state
                        .validated_operations
                        .outdated
                        .retain(|v| v.hash.ne(op));
                    for peer_state in mempool_state.peer_state.values_mut() {
                        peer_state.seen_operations.remove(op);
                    }

                    // remove operation from stats
                    mempool_state.operation_stats.remove(op);
                }
            }
            mempool_state.level_to_operation.retain(|x, _| *x >= level);
            mempool_state.operations_state.retain(|_, operation| {
                level - operation.level < OPERATION_STATUS_RETAIN_LEVELS
                    && operation
                        .operation_decoded_contents
                        .as_ref()
                        .map_or(true, |c| c.is_endorsement() || c.is_preendorsement())
            });
            slog::debug!(state.log, "validated_operations: removed old operations"; "time" => format!("{:?}", Instant::now() - t));

            // reset quorum state.
            let threshold = match state.current_head.constants() {
                Some(v) => v.consensus_threshold,
                None => return,
            };
            state.mempool.prequorum.reset(Some(threshold));
            state.mempool.quorum.reset(Some(threshold));
        }
        Action::MempoolRecvDone(MempoolRecvDoneAction {
            address,
            block_hash: _,
            block_header,
            message,
        }) => {
            let level = block_header.level();
            let pending = message.pending().iter().cloned();
            let known_valid = message.known_valid().iter().cloned();

            let peer = mempool_state.peer_state.entry(*address).or_default();
            let ops = mempool_state.level_to_operation.entry(level).or_default();

            for hash in pending.chain(known_valid) {
                let known = mempool_state.pending_operations.contains_key(&hash)
                    || mempool_state.prechecking_operations.contains_key(&hash)
                    || mempool_state.validated_operations.ops.contains_key(&hash);

                if !known {
                    ops.push(hash.clone());
                    if !mempool_state.pending_full_content.contains_key(&hash) {
                        peer.requesting_full_content.insert(hash.clone());
                        mempool_state
                            .operations_state
                            .insert(hash.clone(), MempoolOperation::received(level, action));
                    }
                }
                // of course peer knows about it, because he sent us it
                peer.seen_operations.insert(hash);
            }
        }
        Action::MempoolMarkOperationsAsPending(MempoolMarkOperationsAsPendingAction {
            address,
            timestamp,
        }) => {
            let peer = mempool_state.peer_state.entry(*address).or_default();
            mempool_state
                .pending_full_content
                .extend_with_timestamp(*timestamp, peer.requesting_full_content.drain());
        }
        Action::MempoolOperationRecvDone(MempoolOperationRecvDoneAction { hash, operation }) => {
            if mempool_state.pending_full_content.remove(hash).is_none() {
                // TODO(vlad): received operation, but we did not requested it, what should we do?
                // We might already processed it.
                return;
            }
            if let Some(head) = state.current_head.get() {
                let proto = head.header.proto();
                if prechecking_enabled(&state.prechecker, proto) && is_consensus_op(operation) {
                    mempool_state
                        .prechecking_operations
                        .insert(hash.clone(), proto);
                    if let Some(operation_state) = mempool_state.operations_state.get_mut(hash) {
                        if let MempoolOperation {
                            state: OperationState::ReceivedHash,
                            ..
                        } = operation_state
                        {
                            *operation_state = operation_state
                                .next_state(OperationState::ReceivedContents, action);
                        }
                    }
                    return;
                }
            }
            mempool_state
                .pending_operations
                .insert(hash.clone(), operation.clone());
            mempool_state.operations_state.remove(hash);
        }
        Action::MempoolOperationInject(MempoolOperationInjectAction {
            operation,
            hash: operation_hash,
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
            if let Some(rpc_id) = rpc_id.as_ref() {
                mempool_state
                    .injecting_rpc_ids
                    .insert(operation_hash.clone(), *rpc_id);
            }

            if let Some(head) = state.current_head.get() {
                let proto = head.header.proto();
                if prechecking_enabled(&state.prechecker, proto) && is_consensus_op(operation) {
                    mempool_state
                        .prechecking_operations
                        .insert(operation_hash.clone(), proto);
                    mempool_state.operations_state.insert(
                        operation_hash.clone(),
                        MempoolOperation::injected(level, *injected_timestamp, action),
                    );
                } else {
                    mempool_state
                        .pending_operations
                        .insert(operation_hash.clone(), operation.clone());
                }
            } else {
                mempool_state
                    .pending_operations
                    .insert(operation_hash.clone(), operation.clone());
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
                .entry(operation_hash.clone())
                .or_insert_with(OperationStats::new)
                .received_via_rpc(
                    &pkh,
                    OperationNodeCurrentHeadStats {
                        time: action.time_as_nanos(),
                        block_level,
                        block_timestamp: block_timestamp.into(),
                    },
                    operation.data().as_ref(),
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
                outdated: act.outdated,
            });
        }
        Action::MempoolUnregisterOperationsStreams(MempoolUnregisterOperationsStreamsAction {}) => {
            mempool_state.operation_streams.clear();
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
        Action::PrecheckerProtocolNeeded(PrecheckerProtocolNeededAction { hash }) => {
            if let Some(operation) = state.prechecker.operation(hash).cloned() {
                let current_head_level = mempool_state
                    .local_head_state
                    .as_ref()
                    .map(|v| v.header.level());

                mempool_state
                    .operation_stats
                    .entry(hash.clone())
                    .or_insert_with(OperationStats::new)
                    .validation_finished(
                        action.time_as_nanos(),
                        None,
                        None,
                        current_head_level,
                        OperationValidationResult::Prevalidate,
                    );

                mempool_state
                    .pending_operations
                    .insert(hash.clone(), operation);
            }
        }
        Action::PrecheckerOperationValidated(PrecheckerOperationValidatedAction { hash }) => {
            mempool_state.prechecking_operations.remove(hash);
            mempool_state.prechecking_delayed_operations.remove(hash);
            let result = if let Some(result) = state.prechecker.result(hash) {
                result
            } else {
                return;
            };

            let current_head_level = mempool_state
                .local_head_state
                .as_ref()
                .map(|v| v.header.level());

            // Ignore consensus operations older than 2 levels.
            if current_head_level.map_or(false, |current_level| {
                result
                    .level()
                    .map_or(false, |level| current_level > level + 2)
            }) {
                return;
            }

            mempool_state
                .validated_operations
                .ops
                .insert(hash.clone(), result.operation().clone());

            if let Some(rpc_id) = mempool_state.injecting_rpc_ids.remove(hash) {
                mempool_state.injected_rpc_ids.push(rpc_id);
            }

            let is_applied = result.kind().is_applied();
            let (state1, state2) = match result.kind() {
                crate::prechecker::PrecheckerResultKind::Applied => {
                    state
                        .mempool
                        .validated_operations
                        .applied
                        .push(as_validated(hash, result));

                    (
                        OperationState::Prechecked,
                        OperationValidationResult::Prechecked,
                    )
                }
                crate::prechecker::PrecheckerResultKind::Outdated => {
                    state
                        .mempool
                        .validated_operations
                        .outdated
                        .push_back(as_errored(hash, result));

                    (
                        OperationState::Outdated,
                        OperationValidationResult::Outdated,
                    )
                }
                crate::prechecker::PrecheckerResultKind::Refused(_) => {
                    state
                        .mempool
                        .validated_operations
                        .refused
                        .push_back(as_errored(hash, result));

                    (OperationState::Refused, OperationValidationResult::Refused)
                }
                crate::prechecker::PrecheckerResultKind::BranchRefused => {
                    state
                        .mempool
                        .validated_operations
                        .branch_refused
                        .push_back(as_errored(hash, result));

                    (
                        OperationState::BranchRefused,
                        OperationValidationResult::BranchRefused,
                    )
                }
                crate::prechecker::PrecheckerResultKind::BranchDelayed => {
                    state
                        .mempool
                        .validated_operations
                        .branch_delayed
                        .push_back(as_errored(hash, result));

                    (
                        OperationState::BranchDelayed,
                        OperationValidationResult::BranchDelayed,
                    )
                }
            };

            if let Some(operation_state) = state.mempool.operations_state.get_mut(hash) {
                if matches!(
                    operation_state.state,
                    OperationState::Decoded | OperationState::BranchDelayed
                ) {
                    *operation_state = operation_state.next_state(state1, action);
                }
                if is_applied {
                    update_quorum_state_with_validated_operation(state, hash);
                }
            }
            state
                .mempool
                .operation_stats
                .entry(hash.clone())
                .or_insert_with(OperationStats::new)
                .validation_finished(
                    action.time_as_nanos(),
                    None,
                    None,
                    current_head_level,
                    state2,
                );
        }
        Action::MempoolBroadcastDone(MempoolBroadcastDoneAction {
            address,
            known_valid,
            pending,
        }) => {
            let peer = mempool_state.peer_state.entry(*address).or_default();

            peer.seen_operations.extend(known_valid.iter().cloned());
            peer.seen_operations.extend(pending.iter().cloned());
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
        Action::MempoolValidatorReady(_) => {
            if mempool_state.branch_changed {
                // remove all `branch_refused` results, put them into `pending_operations`
                // to validate again with new prevalidator
                for v in mem::take(&mut mempool_state.validated_operations.branch_refused) {
                    if let Some(op) = mempool_state.validated_operations.ops.remove(&v.hash) {
                        mempool_state.pending_operations.insert(v.hash, op);
                    }
                }
            }
            // remove all remaining `applied` results and all `branch_delayed` results,
            // put them into `pending_operations` to validate again with new prevalidator
            for v in mem::take(&mut mempool_state.validated_operations.branch_delayed) {
                if let Some(op) = mempool_state.validated_operations.ops.remove(&v.hash) {
                    mempool_state.pending_operations.insert(v.hash, op);
                }
            }
            for v in mem::take(&mut mempool_state.validated_operations.applied) {
                if let Some(op) = mempool_state.validated_operations.ops.remove(&v.hash) {
                    mempool_state.pending_operations.insert(v.hash, op);
                }
            }
        }
        Action::MempoolValidatorValidateInit(content) => {
            let current_head_level = state.current_head.get().map(|v| v.header.level());
            mempool_state
                .operation_stats
                .entry(content.op_hash.clone())
                .or_insert_with(OperationStats::new)
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
                    let block_hash = block_header.message_typed_hash().ok();
                    let mempool = msg.current_mempool();
                    let op_hash_iter = mempool
                        .pending()
                        .iter()
                        .chain(mempool.known_valid())
                        .cloned();

                    for op_hash in op_hash_iter {
                        mempool_state
                            .operation_stats
                            .entry(op_hash)
                            .or_insert_with(OperationStats::new)
                            .received_in_current_head(
                                peer_pkh,
                                block_hash.clone(),
                                OperationNodeCurrentHeadStats {
                                    time,
                                    block_level: block_header.level(),
                                    block_timestamp: block_header.timestamp().into(),
                                },
                            );
                    }
                }
                PeerMessage::GetOperations(msg) => {
                    for op_hash in msg.get_operations().iter().cloned() {
                        mempool_state
                            .operation_stats
                            .entry(op_hash)
                            .or_insert_with(OperationStats::new)
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
                        .or_insert_with(OperationStats::new)
                        .content_received(peer_pkh, time, msg.operation().data().as_ref());
                }
                _ => {}
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

        Action::MempoolGetOperationTimeout(MempoolGetOperationTimeoutAction { timestamp }) => {
            let timeout_timestamp = if let Ok(v) =
                u64::try_from(state.config.mempool_get_operation_timeout.as_nanos())
            {
                timestamp.saturating_sub(v)
            } else {
                return;
            };
            let drain = mempool_state
                .pending_full_content
                .drain_older_than(timeout_timestamp);
            mempool_state.retrying_full_content = drain
                .map(|(hash, _)| {
                    let peers = mempool_state
                        .peer_state
                        .iter()
                        .filter_map(|(addr, peer_state)| {
                            if peer_state.seen_operations.contains(&hash) {
                                Some(addr)
                            } else {
                                None
                            }
                        })
                        .cloned()
                        .collect();
                    (hash, peers)
                })
                .collect();
        }

        Action::MempoolRequestFullContent(MempoolRequestFullContentAction {
            address,
            operations,
        }) => {
            if let Some(peer_state) = mempool_state.peer_state.get_mut(address) {
                peer_state
                    .requesting_full_content
                    .extend(operations.clone());
            }
            for hash in operations {
                mempool_state.retrying_full_content.remove(hash);
            }
        }

        Action::RightsValidatorsReady(_) => {
            if state.mempool.prequorum.total != 0 || state.mempool.quorum.total != 0 {
                // if we have already incremented quorum total endorsing
                // power state, that means we already had rights so we
                // don't need to fill quorum state with the operations
                // that were validated before we had rights available.
                return;
            }
            let ops = &state.mempool.validated_operations;
            let consensus_ops = ops
                .applied
                .iter()
                .map(|v| &v.hash)
                .filter_map(|hash| Some((hash, ops.ops.get(hash)?)))
                .filter(|(_, op)| {
                    OperationKind::from_operation_content_raw(op.data().as_ref())
                        .is_consensus_operation()
                })
                .map(|(hash, _)| hash.clone())
                .collect::<Vec<_>>();
            for hash in consensus_ops {
                update_quorum_state_with_validated_operation(state, &hash);
            }
        }
        Action::MempoolPrequorumReached(_) => {
            state.mempool.prequorum.set_notified();
        }
        Action::MempoolQuorumReached(_) => {
            state.mempool.quorum.set_notified();
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
                    .entry(op_hash)
                    .or_insert_with(OperationStats::new)
                    .sent_in_current_head(
                        pkh,
                        OperationNodeCurrentHeadStats {
                            time,
                            block_level: block_header.level(),
                            block_timestamp: block_header.timestamp().into(),
                        },
                    );
            }
        }
        PeerMessage::GetOperations(msg) => {
            for op_hash in msg.get_operations().iter().cloned() {
                state
                    .mempool
                    .operation_stats
                    .entry(op_hash)
                    .or_insert_with(OperationStats::new)
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
                .entry(op_hash)
                .or_insert_with(OperationStats::new)
                .content_sent(&peer.public_key_hash, time);
        }
        _ => {}
    };
}

fn as_validated(hash: &OperationHash, result: PrecheckerResult) -> Validated {
    Validated {
        hash: hash.clone(),
        protocol_data_json: result
            .contents()
            .map_or(serde_json::Value::Null, |contents| contents.as_json())
            .to_string(),
    }
}

fn as_errored(hash: &OperationHash, result: PrecheckerResult) -> Errored {
    Errored {
        hash: hash.clone(),
        is_endorsement: true,
        protocol_data_json: result
            .contents()
            .map_or(serde_json::Value::Null, |contents| contents.as_json())
            .to_string(),
        error_json: result.error().unwrap_or_else(|| "<no error>".to_string()),
    }
}

fn is_consensus_op(op: &Operation) -> bool {
    matches!(
        OperationKind::from_operation_content_raw(op.data().as_ref()),
        OperationKind::Preendorsement | OperationKind::Endorsement
    )
}

fn drain_consensus_deq<T: HasOperationHash>(
    deq: &mut VecDeque<T>,
    ops: &BTreeMap<OperationHash, Operation>,
) -> VecDeque<T> {
    let (consensus, non_consensus) = deq
        .drain(..)
        .partition(|v| ops.get(v.operation_hash()).map_or(false, is_consensus_op));
    *deq = non_consensus;
    consensus
}

fn drain_consensus<T: HasOperationHash>(
    deq: &mut Vec<T>,
    ops: &BTreeMap<OperationHash, Operation>,
) -> Vec<T> {
    let (consensus, non_consensus) = deq
        .drain(..)
        .partition(|v| ops.get(v.operation_hash()).map_or(false, is_consensus_op));
    *deq = non_consensus;
    consensus
}

fn update_quorum_state_with_validated_operation(
    state: &mut State,
    hash: &OperationHash,
) -> Option<()> {
    let operation_state = state.mempool.operations_state.get(hash)?;
    let op = operation_state
        .operation_decoded_contents
        .as_ref()
        .and_then(|op| match op {
            OperationDecodedContents::Proto012(operation) => Some(operation),
            _ => None,
        })?;

    let (level, round, slot) = op
        .as_preendorsement()
        .map(|op| (op.level, op.round, op.slot))
        .or_else(|| {
            let op = op.as_endorsement()?;
            Some((op.level, op.round, op.slot))
        })?;

    let head_level = state.current_head.level()?;
    let head_round = state.current_head.round()?;
    if level != head_level || round != head_round {
        return None;
    }
    let rights = state.rights.tenderbake_validators(level)?;
    let delegate = rights.validators.get(slot as usize)?;
    let endorsing_power = rights.slots.get(delegate)?.len() as u16;

    if op.is_preendorsement() {
        state
            .mempool
            .prequorum
            .add(delegate.clone(), endorsing_power);
    } else {
        state.mempool.quorum.add(delegate.clone(), endorsing_power);
    }
    Some(())
}
