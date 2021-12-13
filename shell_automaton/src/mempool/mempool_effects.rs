// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use redux_rs::Store;
use std::sync::Arc;

use crypto::hash::OperationHash;

use tezos_messages::p2p::{
    binary_message::MessageHash,
    encoding::{
        current_head::{CurrentHeadMessage, GetCurrentHeadMessage},
        mempool::Mempool,
        operation::{GetOperationsMessage, OperationMessage},
        peer::{PeerMessage, PeerMessageResponse},
    },
};

use tezos_api::ffi::{BeginConstructionRequest, ValidateOperationRequest};

use crate::peer::message::{read::PeerMessageReadSuccessAction, write::PeerMessageWriteInitAction};
use crate::protocol::ProtocolAction;
use crate::storage::kv_operations;
use crate::{
    service::{ProtocolService, RpcService},
    Action, ActionWithMeta, Service, State,
};

use super::{
    mempool_actions::{
        BlockAppliedAction, MempoolBroadcastAction, MempoolBroadcastDoneAction,
        MempoolGetOperationsAction, MempoolGetOperationsPendingAction,
        MempoolOperationInjectAction, MempoolOperationRecvDoneAction, MempoolRecvDoneAction,
        MempoolRpcRespondAction, MempoolValidateStartAction, MempoolValidateWaitPrevalidatorAction,
        MempoolCleanupWaitPrevalidatorAction, MempoolSendAction, MempoolAskCurrentHeadAction,
        MempoolUnregisterOperationsStreamsAction, MempoolFlushAction,
        MempoolRemoveAppliedOperationsAction, BranchChangedAction,
    },
    monitored_operation::MonitoredOperation,
};

pub fn mempool_effects<S>(
    store: &mut Store<State, S, Action>,
    action: &ActionWithMeta,
) where
    S: Service,
{
    // println!("{:#?}", action);
    if store.state().config.disable_mempool {
        if let Action::MempoolOperationInject(MempoolOperationInjectAction { rpc_id, .. }) = &action.action {
            let json = serde_json::Value::String("mempool disabled".to_string());
            store.service().rpc().respond(rpc_id.clone(), json);
        }
        return;
    }
    match &action.action {
        Action::StorageOperationsOk(kv_operations::StorageOperationsOkAction { key, value }) => {
            if store.state().mempool.local_head_state.as_ref().unwrap().1.eq(key) {
                let operation_hashes = value
                    .iter()
                    .map(|op| op.message_typed_hash::<OperationHash>().unwrap())
                    .collect();
                store.dispatch(MempoolRemoveAppliedOperationsAction { operation_hashes });
            }
        }
        Action::Protocol(act) => {
            match act {
                ProtocolAction::PrevalidatorForMempoolReady(_) => {
                    store.dispatch(MempoolFlushAction {});
                    let ops = store.state().mempool.wait_prevalidator_operations.clone();
                    for operation in ops {
                        store.dispatch(MempoolValidateStartAction { operation });
                    }
                    store.dispatch(MempoolCleanupWaitPrevalidatorAction {});
                }
                ProtocolAction::OperationValidated(response) => {
                    store.dispatch(MempoolBroadcastAction {});
                    // respond
                    let ids = store.state().mempool.injected_rpc_ids.clone();
                    for rpc_id in ids {
                        store.service().rpc().respond(rpc_id, serde_json::Value::Null);
                    }
                    store.dispatch(MempoolRpcRespondAction {});
                    let streams = store.state().mempool.operation_streams.clone();
                    for stream in streams {
                        let ops = &store.state().mempool.validated_operations.ops;
                        let refused_ops = &store.state().mempool.validated_operations.refused_ops;
                        let prot = store.state().mempool.prevalidator.as_ref().unwrap().protocol.to_base58_check();
                        let applied = if stream.applied {
                            response.result.applied.as_slice()
                        } else {
                            &[]
                        };
                        let refused = if stream.refused {
                            response.result.refused.as_slice()
                        } else {
                            &[]
                        };
                        let branch_delayed = if stream.branch_delayed {
                            response.result.branch_delayed.as_slice()
                        } else {
                            &[]
                        };
                        let branch_refused = if stream.branch_refused {
                            response.result.branch_refused.as_slice()
                        } else {
                            &[]
                        };
                        let resp = std::iter::empty()
                            .chain(MonitoredOperation::collect_applied(applied, ops, &prot))
                            .chain(MonitoredOperation::collect_errored(refused, refused_ops, &prot))
                            .chain(MonitoredOperation::collect_errored(branch_delayed, ops, &prot))
                            .chain(MonitoredOperation::collect_errored(branch_refused, ops, &prot))
                            .collect::<Vec<_>>();
                        let json = serde_json::to_value(resp).unwrap();
                        store.service().rpc().respond_stream(stream.rpc_id, Some(json));
                    }
                }
                _ => {}
            }
        }
        Action::PeerMessageReadSuccess(PeerMessageReadSuccessAction { message, address }) => {
            match message.message() {
                PeerMessage::GetCurrentHead(ref get_current_head) => {
                    if get_current_head.chain_id().ne(&store.state().config.chain_id) {
                        return;
                    }
                    store.dispatch(MempoolSendAction { address: *address });
                }
                PeerMessage::CurrentHead(ref current_head) => {
                    if store.state().mempool.running_since.is_none() {
                        return;
                    }
                    if current_head.chain_id().ne(&store.state().config.chain_id) {
                        return;
                    }

                    let message = current_head.current_mempool().clone();
                    store.dispatch(
                        MempoolRecvDoneAction {
                            address: *address,
                            message,
                        },
                    );
                }
                PeerMessage::Operation(ref op) => {
                    store.dispatch(
                        MempoolOperationRecvDoneAction {
                            address: *address,
                            operation: op.clone().into(),
                        },
                    );
                }
                PeerMessage::GetOperations(ref hashes) => {
                    for hash in hashes.get_operations() {
                        let mempool = &store.state().mempool;
                        let op = None
                            .or_else(|| mempool.validated_operations.ops.get(hash))
                            .or_else(|| mempool.pending_operations.get(hash));

                        if let Some(op) = op {
                            let message = OperationMessage::from(op.clone());
                            store.dispatch(
                                PeerMessageWriteInitAction {
                                    address: *address,
                                    message: message.into(),
                                },
                            );
                        }
                    }
                }
                _ => (),
            }
        }
        Action::BlockApplied(BlockAppliedAction {
            chain_id, block, block_metadata_hash, ops_metadata_hash, hash, ..
        }) => {
            if store.state().mempool.running_since.is_some() {
                let req = BeginConstructionRequest {
                    chain_id: chain_id.clone(),
                    predecessor: block.clone(),
                    protocol_data: None,
                    predecessor_block_metadata_hash: block_metadata_hash.clone(),
                    predecessor_ops_metadata_hash: ops_metadata_hash.clone(),
                };
                store
                    .service()
                    .protocol()
                    .begin_construction_for_mempool(req);
                if store.state().mempool.branch_changed {
                    store.dispatch(BranchChangedAction {});
                } else {
                    store.dispatch(MempoolBroadcastAction {});
                }
                store.dispatch(kv_operations::StorageOperationsGetAction { key: hash.clone() });
            }
            // close streams
            let streams = store.state().mempool.operation_streams.clone();
            store.dispatch(MempoolUnregisterOperationsStreamsAction {});
            for stream in streams {
                store.service().rpc().respond_stream(stream.rpc_id, None);
            }
        }
        Action::MempoolRegisterOperationsStream(act) => {
            // TODO(vlad): duplicated code
            let ops = &store.state().mempool.validated_operations.ops;
            let refused_ops = &store.state().mempool.validated_operations.refused_ops;
            let prot = store.state().mempool.prevalidator.as_ref().unwrap().protocol.to_base58_check();
            let applied = if act.applied {
                store.state().mempool.validated_operations.applied.as_slice()
            } else {
                &[]
            };
            let refused = if act.refused {
                store.state().mempool.validated_operations.refused.as_slice()
            } else {
                &[]
            };
            let branch_delayed = if act.branch_delayed {
                store.state().mempool.validated_operations.branch_delayed.as_slice()
            } else {
                &[]
            };
            let branch_refused = if act.branch_refused {
                store.state().mempool.validated_operations.branch_refused.as_slice()
            } else {
                &[]
            };
            let resp = std::iter::empty()
                .chain(MonitoredOperation::collect_applied(applied, ops, &prot))
                .chain(MonitoredOperation::collect_errored(refused, refused_ops, &prot))
                .chain(MonitoredOperation::collect_errored(branch_delayed, ops, &prot))
                .chain(MonitoredOperation::collect_errored(branch_refused, ops, &prot))
                .collect::<Vec<_>>();
            let json = serde_json::to_value(resp).unwrap();
            store.service().rpc().respond_stream(act.rpc_id, Some(json));
        }
        Action::MempoolRecvDone(MempoolRecvDoneAction { address, .. }) => {
            if let Some(peer) = store.state().mempool.peer_state.get(address) {
                if !peer.requesting_full_content.is_empty() {
                    store.dispatch(
                        MempoolGetOperationsAction {
                            address: *address,
                        },
                    );
                } else {
                    // if this mempool doesn't introduce new operations, we have nothing to do
                }
            }
        }
        Action::MempoolGetOperations(MempoolGetOperationsAction { address }) => {
            if let Some(peer) = store.state().mempool.peer_state.get(address) {
                let ops = peer.requesting_full_content.iter().cloned().collect();
                store.dispatch(
                    MempoolGetOperationsPendingAction {
                        address: *address,
                    },
                );
                store.dispatch(
                    PeerMessageWriteInitAction {
                        address: *address,
                        message: Arc::new(GetOperationsMessage::new(ops).into()),
                    },
                );
            }
        }
        Action::MempoolOperationRecvDone(MempoolOperationRecvDoneAction { operation, .. })
        | Action::MempoolOperationInject(MempoolOperationInjectAction { operation, .. }) => {
            store
                .dispatch(
                    MempoolValidateStartAction {
                        operation: operation.clone(),
                    },
                );
        }
        Action::MempoolValidateStart(MempoolValidateStartAction { operation }) => {
            let mempool_state = &store.state().mempool;
            if let Some(prevalidator) = &mempool_state.prevalidator {
                let validate_req = ValidateOperationRequest {
                    prevalidator: prevalidator.clone(),
                    operation: operation.clone(),
                };
                store
                    .service()
                    .protocol()
                    .validate_operation_for_mempool(validate_req);
            } else {
                store.dispatch(MempoolValidateWaitPrevalidatorAction { operation: operation.clone() });
            }
        }
        Action::MempoolBroadcast(MempoolBroadcastAction {}) => {
            let addresses = store.state().peers.iter_addr().cloned().collect::<Vec<_>>();
            // TODO(vlad): add action removing peer_state for disconnected peers
            for address in addresses {
                store.dispatch(MempoolSendAction { address });
            }
        }
        Action::MempoolAskCurrentHead(MempoolAskCurrentHeadAction {}) => {
            let addresses = store.state().peers.iter_addr().cloned().collect::<Vec<_>>();
            let message = GetCurrentHeadMessage::new(
                store.state().config.chain_id.clone(),
            );
            let message = Arc::new(PeerMessageResponse::from(message));
            for address in addresses {
                store.dispatch(
                    PeerMessageWriteInitAction {
                        address: address.clone(),
                        message: message.clone(),
                    },
                );
            }
        }
        Action::MempoolSend(MempoolSendAction { address }) => {
            let (head_state, head_hash) = match store.state().mempool.local_head_state.clone() {
                Some(v) => v,
                None => {
                    // should always have current head here
                    // TODO(vlad): should be forbidden by enabling condition
                    return;
                }
            };
            let peer = match store.state().mempool.peer_state.get(&address) {
                Some(v) => v,
                None => return,
            };

            let known_valid = store
                .state()
                .mempool
                .validated_operations
                .ops
                .iter()
                .filter(|(hash, op)| {
                    !peer.seen_operations.contains(*hash) && head_hash.eq(op.branch())
                })
                .map(|(hash, _)| hash)
                .cloned()
                .collect::<Vec<_>>();
            let pending = store
                .state()
                .mempool
                .pending_operations
                .iter()
                .filter(|(hash, op)| {
                    !peer.seen_operations.contains(*hash) && head_hash.eq(op.branch())
                })
                .map(|(hash, _)| hash)
                .cloned()
                .collect::<Vec<_>>();
            let message = CurrentHeadMessage::new(
                store.state().config.chain_id.clone(),
                head_state.clone(),
                Mempool::new(known_valid.clone(), pending.clone()),
            );
            let message = Arc::new(PeerMessageResponse::from(message));

            store.dispatch(
                PeerMessageWriteInitAction {
                    address: address.clone(),
                    message,
                },
            );
            store.dispatch(
                MempoolBroadcastDoneAction {
                    address: address.clone(),
                    pending,
                    known_valid,
                },
            );
        }
        _ => (),
    }
}
