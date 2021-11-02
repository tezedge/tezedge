// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::mem;

use tezos_messages::p2p::binary_message::MessageHash;

use crate::protocol::ProtocolAction;
use crate::peers::remove::PeersRemoveAction;
use crate::{Action, ActionWithMeta, State};

use super::{
    BlockAppliedAction, MempoolBroadcastDoneAction, MempoolMarkOperationsAsPendingAction,
    MempoolOperationInjectAction, MempoolOperationRecvDoneAction, MempoolRecvDoneAction,
    MempoolRpcRespondAction, MempoolValidateWaitPrevalidatorAction, MempoolCleanupWaitPrevalidatorAction,
    MempoolSendAction, MempoolUnregisterOperationsStreamsAction, MempoolRemoveAppliedOperationsAction,
    BranchChangedAction,
    mempool_state::OperationStream,
};

pub fn mempool_reducer(state: &mut State, action: &ActionWithMeta) {
    if state.config.disable_mempool {
        return;
    }
    let State { config, mempool, .. } = state;
    let config = &*config;
    let mempool_state = mempool;

    match &action.action {
        Action::Protocol(act) => match act {
            ProtocolAction::PrevalidatorForMempoolReady(prevalidator) => {
                mempool_state.prevalidator = Some(prevalidator.clone());
            }
            ProtocolAction::OperationValidated(result) => {
                mempool_state.prevalidator = Some(result.prevalidator.clone());
                for v in &result.result.applied {
                    if let Some(op) = mempool_state.pending_operations.remove(&v.hash) {
                        mempool_state
                            .validated_operations
                            .ops
                            .insert(v.hash.clone(), op);
                        mempool_state.validated_operations.applied.push(v.clone());
                    }
                    if let Some(rpc_id) = mempool_state.injecting_rpc_ids.remove(&v.hash) {
                        mempool_state.injected_rpc_ids.push(rpc_id);
                    }
                }
                for v in &result.result.refused {
                    if let Some(op) = mempool_state.pending_operations.remove(&v.hash) {
                        mempool_state
                            .validated_operations
                            .refused_ops
                            .insert(v.hash.clone(), op);
                        mempool_state.validated_operations.refused.push(v.clone());
                    }
                    if let Some(rpc_id) = mempool_state.injecting_rpc_ids.remove(&v.hash) {
                        mempool_state.injected_rpc_ids.push(rpc_id);
                    }
                }
                for v in &result.result.branch_refused {
                    if let Some(op) = mempool_state.pending_operations.remove(&v.hash) {
                        mempool_state
                            .validated_operations
                            .ops
                            .insert(v.hash.clone(), op);
                        mempool_state
                            .validated_operations
                            .branch_refused
                            .push(v.clone());
                    }
                    if let Some(rpc_id) = mempool_state.injecting_rpc_ids.remove(&v.hash) {
                        mempool_state.injected_rpc_ids.push(rpc_id);
                    }
                }
                for v in &result.result.branch_delayed {
                    if let Some(op) = mempool_state.pending_operations.remove(&v.hash) {
                        mempool_state
                            .validated_operations
                            .ops
                            .insert(v.hash.clone(), op);
                        mempool_state
                            .validated_operations
                            .branch_delayed
                            .push(v.clone());
                    }
                    if let Some(rpc_id) = mempool_state.injecting_rpc_ids.remove(&v.hash) {
                        mempool_state.injected_rpc_ids.push(rpc_id);
                    }
                }
            }
            act => {
                let _ = act;
                // println!("{:?}", act);
            }
        },
        Action::BlockApplied(BlockAppliedAction {
            chain_id, block, hash, is_bootstrapped, ..
        }) => {
            if config.chain_id.ne(chain_id) {
                return;
            }
            if *is_bootstrapped {
                mempool_state.running_since = Some(());
            }
            let changed = if let Some((_, old_head_hash)) = &mempool_state.local_head_state {
                old_head_hash.ne(&block.predecessor())
            } else {
                false
            };
            mempool_state.branch_changed = changed;
            mempool_state.local_head_state = Some((block.clone(), hash.clone()));
            mempool_state.applied_block.insert(hash.clone());

            // TODO(vlad) move to separate action
            // TODO: get from protocol
            const TTL: i32 = 120;
            if block.level() < TTL {
                return;
            }
            let level = block.level() - TTL;

            // `drain_filter` is unstable for now
            for (_, ops) in mempool_state.level_to_operation.range(..level) {
                for op in ops {
                    mempool_state.pending_operations.remove(op);
                    mempool_state.validated_operations.ops.remove(op);
                    mempool_state.validated_operations.refused_ops.remove(op);
                    mempool_state.validated_operations.applied.retain(|v| v.hash.ne(op));
                    mempool_state.validated_operations.refused.retain(|v| v.hash.ne(op));
                    mempool_state.validated_operations.branch_delayed.retain(|v| v.hash.ne(op));
                    mempool_state.validated_operations.branch_refused.retain(|v| v.hash.ne(op));
                    for (_, peer_state) in &mut mempool_state.peer_state {
                        peer_state.seen_operations.remove(op);
                    }
                }
            }
            mempool_state.level_to_operation.retain(|x, _| *x >= level);
        }
        Action::MempoolRecvDone(MempoolRecvDoneAction {
            address,
            message,
            level,
        }) => {
            let pending = message.pending().iter().cloned();
            let known_valid = message.known_valid().iter().cloned();

            let peer = mempool_state.peer_state.entry(*address).or_default();
            let ops = mempool_state.level_to_operation.entry(*level).or_default();
            for hash in pending.chain(known_valid) {
                let known = mempool_state.pending_operations.contains_key(&hash)
                    || mempool_state.validated_operations.ops.contains_key(&hash);
                if !known {
                    ops.push(hash.clone());
                    peer.requesting_full_content.insert(hash.clone());
                    // of course peer knows about it, because he sent us it
                    peer.seen_operations.insert(hash);
                }
            }
        }
        Action::MempoolMarkOperationsAsPending(MempoolMarkOperationsAsPendingAction { address }) => {
            let peer = mempool_state.peer_state.entry(*address).or_default();
            peer.pending_full_content
                .extend(peer.requesting_full_content.drain());
        }
        Action::MempoolOperationRecvDone(MempoolOperationRecvDoneAction { address, operation }) => {
            let operation_hash = match operation.message_typed_hash() {
                Ok(v) => v,
                Err(err) => {
                    // TODO(vlad): peer send bad operation, should log the error,
                    // maybe should disconnect the peer
                    let _ = err;
                    return;
                }
            };
            let peer = mempool_state.peer_state.entry(*address).or_default();

            if !peer.pending_full_content.remove(&operation_hash) {
                // TODO(vlad): received operation, but we did not requested it, what should we do?
            }

            mempool_state
                .pending_operations
                .insert(operation_hash, operation.clone());
        }
        Action::MempoolOperationInject(MempoolOperationInjectAction {
            operation,
            operation_hash,
            rpc_id,
        }) => {
            let level = mempool_state
                .local_head_state
                .as_ref()
                .map(|(h, _)| h.level())
                .unwrap_or(0);
            let ops = mempool_state.level_to_operation.entry(level).or_default();
            ops.push(operation_hash.clone());
            mempool_state
                .injecting_rpc_ids
                .insert(operation_hash.clone(), rpc_id.clone());
            mempool_state
                .pending_operations
                .insert(operation_hash.clone(), operation.clone());
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
        },
        Action::MempoolValidateWaitPrevalidator(MempoolValidateWaitPrevalidatorAction {
            operation,
        }) => {
            mempool_state.wait_prevalidator_operations.push(operation.clone());
        }
        Action::MempoolCleanupWaitPrevalidator(MempoolCleanupWaitPrevalidatorAction {}) => {
            mempool_state.wait_prevalidator_operations.clear();
        }
        Action::MempoolRpcRespond(MempoolRpcRespondAction {}) => {
            mempool_state.injected_rpc_ids.clear();
        }
        Action::MempoolSend(MempoolSendAction { address }) => {
            mempool_state.peer_state.entry(*address).or_default();
        }
        Action::PeersRemove(PeersRemoveAction { address }) => {
            mempool_state.peer_state.remove(address);
        }
        Action::MempoolBroadcastDone(MempoolBroadcastDoneAction {
            address,
            known_valid,
            pending,
        }) => {
            let peer = mempool_state.peer_state.entry(*address).or_default();

            peer.seen_operations.extend(known_valid.iter().cloned());
            peer.seen_operations.extend(pending.iter().cloned());
        }
        Action::MempoolRemoveAppliedOperations(MempoolRemoveAppliedOperationsAction { operation_hashes }) => {
            mempool_state.validated_operations.applied.retain(|v| !operation_hashes.contains(&v.hash));
            mempool_state.validated_operations.branch_delayed.retain(|v| !operation_hashes.contains(&v.hash));
            for op in operation_hashes {
                mempool_state.validated_operations.ops.remove(op);
            }
        }
        Action::BranchChanged(BranchChangedAction {}) => {
            // remove all `branch_refused` results, put them into `pending_operations` collections
            // to validate again with new prevalidator
            for v in mem::take(&mut mempool_state.validated_operations.branch_refused) {
                if let Some(op) = mempool_state.validated_operations.ops.remove(&v.hash) {
                    mempool_state.pending_operations.insert(v.hash, op.clone());
                    mempool_state.wait_prevalidator_operations.push(op);
                }
            }
        }
        _ => (),
    }
}
