// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crypto::hash::BlockHash;
use tezos_messages::p2p::binary_message::MessageHash;

use crate::mempool::mempool_state::OperationState;
use crate::prechecker::{
    PrecheckerPrecheckOperationResponse, PrecheckerPrecheckOperationResponseAction,
};
use crate::protocol::ProtocolAction;
use crate::{Action, ActionWithMeta, State};

use super::{
    BlockAppliedAction, HeadState, MempoolBroadcastDoneAction, MempoolGetOperationsPendingAction,
    MempoolOperationInjectAction, MempoolOperationPrecheckedAction, MempoolOperationRecvDoneAction,
    MempoolRecvDoneAction, MempoolRpcRespondAction,
};

pub fn mempool_reducer(state: &mut State, action: &ActionWithMeta) {
    if state.config.disable_mempool {
        return;
    }
    let mut mempool_state = &mut state.mempool;

    match &action.action {
        Action::Protocol(act) => match act {
            ProtocolAction::PrevalidatorForMempoolReady(prevalidator) => {
                mempool_state.prevalidator = Some(prevalidator.clone());
            }
            ProtocolAction::OperationValidated(result) => {
                mempool_state.prevalidator = Some(result.prevalidator.clone());
                for v in &result.result.applied {
                    mempool_state.wait_prevalidator_operations.remove(&v.hash);
                    if let Some(op) = mempool_state.pending_operations.remove(&v.hash) {
                        mempool_state
                            .validated_operations
                            .ops
                            .insert(v.hash.clone().into(), op);
                        mempool_state.validated_operations.applied.push(v.clone());
                    }
                    if let Some(rpc_id) = mempool_state.injecting_rpc_ids.remove(&v.hash) {
                        mempool_state
                            .injected_rpc_ids
                            .insert(v.hash.clone().into(), rpc_id);
                    }
                }
                for v in &result.result.refused {
                    mempool_state.wait_prevalidator_operations.remove(&v.hash);
                    if let Some(op) = mempool_state.pending_operations.remove(&v.hash) {
                        mempool_state
                            .validated_operations
                            .refused_ops
                            .insert(v.hash.clone().into(), op);
                        mempool_state.validated_operations.refused.push(v.clone());
                    }
                    if let Some(rpc_id) = mempool_state.injecting_rpc_ids.remove(&v.hash) {
                        mempool_state
                            .injected_rpc_ids
                            .insert(v.hash.clone().into(), rpc_id);
                    }
                }
                for v in &result.result.branch_refused {
                    mempool_state.wait_prevalidator_operations.remove(&v.hash);
                    if let Some(op) = mempool_state.pending_operations.remove(&v.hash) {
                        mempool_state
                            .validated_operations
                            .ops
                            .insert(v.hash.clone().into(), op);
                        mempool_state
                            .validated_operations
                            .branch_refused
                            .push(v.clone());
                    }
                    if let Some(rpc_id) = mempool_state.injecting_rpc_ids.remove(&v.hash) {
                        mempool_state
                            .injected_rpc_ids
                            .insert(v.hash.clone().into(), rpc_id);
                    }
                }
                for v in &result.result.branch_delayed {
                    mempool_state.wait_prevalidator_operations.remove(&v.hash);
                    if let Some(op) = mempool_state.pending_operations.remove(&v.hash) {
                        mempool_state
                            .validated_operations
                            .ops
                            .insert(v.hash.clone().into(), op);
                        mempool_state
                            .validated_operations
                            .branch_delayed
                            .push(v.clone());
                    }
                    if let Some(rpc_id) = mempool_state.injecting_rpc_ids.remove(&v.hash) {
                        mempool_state
                            .injected_rpc_ids
                            .insert(v.hash.clone().into(), rpc_id);
                    }
                }
            }
            act => {
                println!("{:?}", act);
            }
        },
        Action::BlockApplied(BlockAppliedAction {
            chain_id, block, ..
        }) => {
            match block.message_typed_hash::<BlockHash>() {
                Ok(hash) => {
                    mempool_state.local_head_state = Some((
                        HeadState {
                            chain_id: chain_id.clone(),
                            current_block: block.clone(),
                        },
                        hash.clone(),
                    ));
                    mempool_state.applied_block.insert(hash);
                }
                Err(err) => {
                    // TODO(vlad): unwrap
                    let _ = err;
                }
            }
        }
        Action::MempoolRecvDone(MempoolRecvDoneAction {
            address,
            message,
            head_state,
        }) => {
            // this is a new block
            if mempool_state
                .local_head_state
                .as_ref()
                .map(|(head, _)| head.current_block.level() != head_state.current_block.level())
                .unwrap_or(false)
            {
                mempool_state.local_head_state = None;
                mempool_state.operations_state.clear();
            }

            let pending = message.pending().iter().cloned();
            let known_valid = message.known_valid().iter().cloned();

            let block_time = match mempool_state.head_timestamp() {
                Some(v) => v,
                None => return,
            };

            let peer = mempool_state.peer_state.entry(*address).or_default();
            for hash in pending.chain(known_valid) {
                let known = mempool_state.pending_operations.contains_key(&hash)
                    || mempool_state.validated_operations.ops.contains_key(&hash);
                if !known {
                    peer.requesting_full_content.insert(hash.clone());
                    // of course peer knows about it, because he sent us it
                    peer.seen_operations.insert(hash.clone());
                    mempool_state.operations_state.insert(
                        hash.into(),
                        OperationState::Received {
                            receive_time: (action.time_as_nanos() - block_time * 1_000_000_000)
                                / 1_000_000_000,
                        },
                    );
                }
            }
        }
        Action::MempoolGetOperationsPending(MempoolGetOperationsPendingAction { address }) => {
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
                .insert(operation_hash.into(), operation.clone());
        }
        Action::MempoolOperationInject(MempoolOperationInjectAction {
            operation,
            operation_hash,
            rpc_id,
        }) => {
            mempool_state
                .injecting_rpc_ids
                .insert(operation_hash.clone().into(), rpc_id.clone());
            mempool_state
                .pending_operations
                .insert(operation_hash.clone().into(), operation.clone());
        }
        Action::PrecheckerPrecheckOperationResponse(
            PrecheckerPrecheckOperationResponseAction { response },
        ) => match response {
            PrecheckerPrecheckOperationResponse::Applied(applied) => {
                if let Some(op) = mempool_state.pending_operations.remove(&applied.hash) {
                    mempool_state
                        .validated_operations
                        .ops
                        .insert(applied.hash.clone().into(), op);
                    mempool_state
                        .validated_operations
                        .applied
                        .push(applied.clone());
                }
                if let Some(rpc_id) = mempool_state.injecting_rpc_ids.remove(&applied.hash) {
                    mempool_state
                        .injected_rpc_ids
                        .insert(applied.hash.clone().into(), rpc_id);
                }
            }
            PrecheckerPrecheckOperationResponse::Refused(errored) => {
                if let Some(op) = mempool_state.pending_operations.remove(&errored.hash) {
                    mempool_state
                        .validated_operations
                        .refused_ops
                        .insert(errored.hash.clone().into(), op);
                    mempool_state
                        .validated_operations
                        .refused
                        .push(errored.clone());
                }
                if let Some(rpc_id) = mempool_state.injecting_rpc_ids.remove(&errored.hash) {
                    mempool_state
                        .injected_rpc_ids
                        .insert(errored.hash.clone().into(), rpc_id);
                }
            }
            _ => (),
        },
        Action::MempoolRpcRespond(MempoolRpcRespondAction {}) => {
            state.mempool.injected_rpc_ids.clear();
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

        Action::MempoolOperationPrechecked(MempoolOperationPrecheckedAction {
            operation,
            protocol_data,
        }) => {
            let block_time = match mempool_state.head_timestamp() {
                Some(v) => v,
                None => return,
            };
            if let Some(operation_state) = mempool_state.operations_state.get_mut(operation) {
                if let OperationState::Received { ref receive_time } = operation_state {
                    *operation_state = OperationState::Prechecked {
                        protocol_data: protocol_data.clone(),
                        receive_time: *receive_time,
                        precheck_time: (action.time_as_nanos() - block_time * 1_000_000_000)
                            / 1_000_000_000,
                    };
                }
            }
        }
        _ => (),
    }
}
