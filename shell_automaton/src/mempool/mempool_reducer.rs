// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::mem;
use std::net::SocketAddr;

use crypto::hash::OperationHash;
use tezos_messages::p2p::binary_message::MessageHash;
use tezos_messages::p2p::encoding::peer::PeerMessage;

use crate::peers::remove::PeersRemoveAction;
use crate::protocol::ProtocolAction;
use crate::{Action, ActionWithMeta, State};

use super::mempool_state::{HeadState, OperationStream};
use super::{
    BlockAppliedAction, MempoolBroadcastDoneAction, MempoolCleanupWaitPrevalidatorAction,
    MempoolFlushAction, MempoolMarkOperationsAsPendingAction, MempoolOperationInjectAction,
    MempoolOperationRecvDoneAction, MempoolRecvDoneAction, MempoolRemoveAppliedOperationsAction,
    MempoolRpcRespondAction, MempoolSendAction, MempoolUnregisterOperationsStreamsAction,
    MempoolValidateWaitPrevalidatorAction, OperationNodeCurrentHeadStats, OperationNodeStats,
    OperationStats, OperationValidationResult,
};

pub fn mempool_reducer(state: &mut State, action: &ActionWithMeta) {
    if state.config.disable_mempool {
        return;
    }
    let config = &state.config;
    let mempool_state = &mut state.mempool;

    match &action.action {
        Action::Protocol(act) => match act {
            ProtocolAction::PrevalidatorForMempoolReady(prevalidator) => {
                mempool_state.prevalidator = Some(prevalidator.clone());
                // unwrap is safe, cannot have prevalidator and haven't `local_head_state`
                mempool_state
                    .local_head_state
                    .as_mut()
                    .unwrap()
                    .prevalidator_ready = true;
            }
            ProtocolAction::OperationValidated(result) => {
                mempool_state.prevalidator = Some(result.prevalidator.clone());
                for v in &result.result.applied {
                    if let Some(op) = mempool_state.pending_operations.remove(&v.hash) {
                        mempool_state
                            .validated_operations
                            .ops
                            .insert(v.hash.clone().into(), op);
                        mempool_state.validated_operations.applied.push(v.clone());
                        mempool_state
                            .operation_stats
                            .entry(v.hash.clone().into())
                            .or_insert_with(|| OperationStats::new())
                            .validation_result =
                            Some((action.time_as_nanos(), OperationValidationResult::Applied));
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
                            .insert(v.hash.clone().into(), op);
                        mempool_state.validated_operations.refused.push(v.clone());
                        mempool_state
                            .operation_stats
                            .entry(v.hash.clone().into())
                            .or_insert_with(|| OperationStats::new())
                            .validation_result =
                            Some((action.time_as_nanos(), OperationValidationResult::Refused));
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
                            .insert(v.hash.clone().into(), op);
                        mempool_state
                            .validated_operations
                            .branch_refused
                            .push(v.clone());
                        mempool_state
                            .operation_stats
                            .entry(v.hash.clone().into())
                            .or_insert_with(|| OperationStats::new())
                            .validation_result = Some((
                            action.time_as_nanos(),
                            OperationValidationResult::BranchRefused,
                        ));
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
                            .insert(v.hash.clone().into(), op);
                        mempool_state
                            .validated_operations
                            .branch_delayed
                            .push(v.clone());
                        mempool_state
                            .operation_stats
                            .entry(v.hash.clone().into())
                            .or_insert_with(|| OperationStats::new())
                            .validation_result = Some((
                            action.time_as_nanos(),
                            OperationValidationResult::BranchDelayed,
                        ));
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
            chain_id,
            block,
            hash,
            is_bootstrapped,
            ..
        }) => {
            if config.chain_id.ne(chain_id) {
                return;
            }
            if *is_bootstrapped {
                mempool_state.running_since = Some(());
            }
            let changed = if let Some(old_head) = &mempool_state.local_head_state {
                old_head.hash.ne(&block.predecessor())
            } else {
                false
            };
            mempool_state.branch_changed = changed;
            mempool_state.local_head_state = Some(HeadState {
                header: block.clone(),
                hash: hash.clone(),
                ops_removed: false,
                prevalidator_ready: false,
            });

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
                    if !mempool_state.pending_full_content.contains(&hash) {
                        peer.requesting_full_content.insert(hash.clone());
                    }
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

            if !mempool_state.pending_full_content.remove(&operation_hash) {
                // TODO(vlad): received operation, but we did not requested it, what should we do?
            }

            mempool_state
                .pending_operations
                .insert(operation_hash.into(), operation.clone());
        }
        Action::MempoolOperationInject(MempoolOperationInjectAction {
            operation,
            operation_hash,
            rpc_id,
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
        Action::MempoolBroadcastDone(MempoolBroadcastDoneAction {
            address,
            known_valid,
            pending,
        }) => {
            let peer = mempool_state.peer_state.entry(*address).or_default();

            peer.seen_operations.extend(known_valid.iter().cloned());
            peer.seen_operations.extend(pending.iter().cloned());
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

                // remove operation from stats
                mempool_state.operation_stats.remove(op);
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
        Action::PeerMessageReadSuccess(content) => {
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
                        .content_received(peer_pkh, time);
                }
                _ => return,
            };
        }
        Action::PeerMessageWriteInit(content) => {
            match content.message.message() {
                PeerMessage::CurrentHead(_)
                | PeerMessage::GetOperations(_)
                | PeerMessage::Operation(_) => {}
                _ => return,
            }
            update_operation_sent_stats(state, content.address, action.time_as_nanos());
        }
        Action::PeerMessageWriteNext(content) => {
            update_operation_sent_stats(state, content.address, action.time_as_nanos());
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
