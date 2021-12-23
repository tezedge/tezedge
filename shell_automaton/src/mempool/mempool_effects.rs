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

use tezos_api::ffi::BeginConstructionRequest;

use crate::storage::kv_operations;
use crate::{
    peer::message::{read::PeerMessageReadSuccessAction, write::PeerMessageWriteInitAction},
    protocol::protocol_actions::*,
};
use crate::{service::RpcService, Action, ActionWithMeta, Service, State};

use super::{
    mempool_actions::*,
    monitored_operation::{MempoolOperations, MonitoredOperation},
};

pub fn mempool_effects<S>(store: &mut Store<State, S, Action>, action: &ActionWithMeta)
where
    S: Service,
{
    if store.state().config.disable_mempool {
        match &action.action {
            Action::MempoolOperationInject(MempoolOperationInjectAction { rpc_id, .. }) => {
                let json = serde_json::Value::String("disabled".to_string());
                store.service().rpc().respond(*rpc_id, json);
            }
            Action::MempoolGetPendingOperations(MempoolGetPendingOperationsAction { rpc_id }) => {
                store
                    .service()
                    .rpc()
                    .respond(*rpc_id, MempoolOperations::default());
            }
            _ => (),
        }
        return;
    }
    match &action.action {
        Action::StorageOperationsOk(kv_operations::StorageOperationsOkAction { key, value }) => {
            if store
                .state()
                .mempool
                .local_head_state
                .as_ref()
                .map(|h| h.hash.eq(&key.0))
                .unwrap_or(false)
            {
                let operation_hashes = value
                    .iter()
                    .filter_map(|op| op.message_typed_hash::<OperationHash>().ok())
                    .collect();
                store.dispatch(MempoolRemoveAppliedOperationsAction { operation_hashes });

                if store.state().mempool.branch_changed {
                    let v = store
                        .state()
                        .mempool
                        .validated_operations
                        .branch_refused
                        .clone();
                    for v in v {
                        if let Some(op) =
                            store.state().mempool.validated_operations.ops.get(&v.hash)
                        {
                            let operation = op.clone();
                            store.dispatch(MempoolValidateStartAction { operation });
                        }
                    }
                } else {
                    let v = store
                        .state()
                        .mempool
                        .validated_operations
                        .branch_delayed
                        .clone();
                    for v in v {
                        if let Some(op) =
                            store.state().mempool.validated_operations.ops.get(&v.hash)
                        {
                            let operation = op.clone();
                            store.dispatch(MempoolValidateStartAction { operation });
                        }
                    }
                    let v = store.state().mempool.validated_operations.applied.clone();
                    for v in v {
                        if let Some(op) =
                            store.state().mempool.validated_operations.ops.get(&v.hash)
                        {
                            let operation = op.clone();
                            store.dispatch(MempoolValidateStartAction { operation });
                        }
                    }
                }

                store.dispatch(MempoolFlushAction {});
            }
        }
        Action::ProtocolValidateOperationDone(ProtocolValidateOperationDoneAction { response }) => {
            let addresses = store.state().peers.iter_addr().cloned().collect::<Vec<_>>();
            for address in addresses {
                store.dispatch(MempoolSendValidatedAction { address });
            }
            // respond
            let ids = store.state().mempool.injected_rpc_ids.clone();
            for rpc_id in ids {
                store
                    .service()
                    .rpc()
                    .respond(rpc_id, serde_json::Value::Null);
            }
            store.dispatch(MempoolRpcRespondAction {});
            let streams = store.state().mempool.operation_streams.clone();
            for stream in streams {
                let ops = &store.state().mempool.validated_operations.ops;
                let refused_ops = &store.state().mempool.validated_operations.refused_ops;
                // `ProtocolValidateOperationDone` action can happens only
                // if we have a prevalidator
                let prot = store
                    .state()
                    .protocol
                    .operation_validation_state
                    .protocol()
                    .unwrap()
                    .to_base58_check();
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
                    .chain(MonitoredOperation::collect_errored(
                        refused,
                        refused_ops,
                        &prot,
                    ))
                    .chain(MonitoredOperation::collect_errored(
                        branch_delayed,
                        ops,
                        &prot,
                    ))
                    .chain(MonitoredOperation::collect_errored(
                        branch_refused,
                        ops,
                        &prot,
                    ))
                    .collect::<Vec<_>>();
                let json = serde_json::to_value(resp).unwrap();
                store
                    .service()
                    .rpc()
                    .respond_stream(stream.rpc_id, Some(json));
            }
        }
        Action::PeerMessageReadSuccess(PeerMessageReadSuccessAction { message, address }) => {
            match message.message() {
                PeerMessage::GetCurrentHead(ref get_current_head) => {
                    if get_current_head
                        .chain_id()
                        .ne(&store.state().config.chain_id)
                    {
                        return;
                    }
                    store.dispatch(MempoolSendAction {
                        address: *address,
                        send_operations: true,
                        requested_explicitly: true,
                    });
                }
                PeerMessage::CurrentHead(ref current_head) => {
                    if store.state().mempool.running_since.is_none() {
                        return;
                    }
                    if current_head.chain_id().ne(&store.state().config.chain_id) {
                        return;
                    }

                    let message = current_head.current_mempool().clone();
                    store.dispatch(MempoolRecvDoneAction {
                        address: *address,
                        message,
                        level: current_head.current_block_header().level(),
                    });
                }
                PeerMessage::Operation(ref op) => {
                    store.dispatch(MempoolOperationRecvDoneAction {
                        operation: op.clone().into(),
                    });
                }
                PeerMessage::GetOperations(ref hashes) => {
                    for hash in hashes.get_operations() {
                        let mempool = &store.state().mempool;
                        let op = None
                            .or_else(|| mempool.validated_operations.ops.get(hash))
                            .or_else(|| mempool.pending_operations.get(hash));

                        if let Some(op) = op {
                            let message = OperationMessage::from(op.clone());
                            store.dispatch(PeerMessageWriteInitAction {
                                address: *address,
                                message: message.into(),
                            });
                        }
                    }
                }
                _ => (),
            }
        }
        Action::BlockApplied(BlockAppliedAction {
            chain_id,
            block,
            block_metadata_hash,
            ops_metadata_hash,
            hash,
            ..
        }) => {
            if store.state().mempool.running_since.is_some() {
                let request = BeginConstructionRequest {
                    chain_id: chain_id.clone(),
                    predecessor: block.clone(),
                    protocol_data: None,
                    predecessor_block_metadata_hash: block_metadata_hash.clone(),
                    predecessor_ops_metadata_hash: ops_metadata_hash.clone(),
                };
                store.dispatch(ProtocolConstructStatelessPrevalidatorStartAction { request });
                store.dispatch(kv_operations::StorageOperationsGetAction {
                    key: hash.clone().into(),
                });
                if !store.state().mempool.branch_changed {
                    store.dispatch(MempoolBroadcastAction {
                        send_operations: false,
                    });
                }
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
            let prot = match store.state().protocol.operation_validation_state.protocol() {
                Some(v) => v.to_base58_check(),
                None => return,
            };
            let applied = if act.applied {
                store
                    .state()
                    .mempool
                    .validated_operations
                    .applied
                    .as_slice()
            } else {
                &[]
            };
            let refused = if act.refused {
                store
                    .state()
                    .mempool
                    .validated_operations
                    .refused
                    .as_slice()
            } else {
                &[]
            };
            let branch_delayed = if act.branch_delayed {
                store
                    .state()
                    .mempool
                    .validated_operations
                    .branch_delayed
                    .as_slice()
            } else {
                &[]
            };
            let branch_refused = if act.branch_refused {
                store
                    .state()
                    .mempool
                    .validated_operations
                    .branch_refused
                    .as_slice()
            } else {
                &[]
            };
            let resp = std::iter::empty()
                .chain(MonitoredOperation::collect_applied(applied, ops, &prot))
                .chain(MonitoredOperation::collect_errored(
                    refused,
                    refused_ops,
                    &prot,
                ))
                .chain(MonitoredOperation::collect_errored(
                    branch_delayed,
                    ops,
                    &prot,
                ))
                .chain(MonitoredOperation::collect_errored(
                    branch_refused,
                    ops,
                    &prot,
                ))
                .collect::<Vec<_>>();
            if let Ok(json) = serde_json::to_value(resp) {
                store.service().rpc().respond_stream(act.rpc_id, Some(json));
            }
        }
        Action::MempoolGetPendingOperations(MempoolGetPendingOperationsAction { rpc_id }) => {
            let empty = MempoolOperations::default();
            let protocol = match store.state().protocol.operation_validation_state.protocol() {
                Some(v) => v,
                None => {
                    store.service().rpc().respond(*rpc_id, empty);
                    return;
                }
            };
            let current_branch = match &store.state().mempool.local_head_state {
                Some(v) => &v.hash,
                None => {
                    store.service().rpc().respond(*rpc_id, empty);
                    return;
                }
            };
            let v_ops = &store.state().mempool.validated_operations;
            let v = MempoolOperations::collect(
                &v_ops.applied,
                &v_ops.refused,
                &v_ops.branch_delayed,
                &v_ops.branch_refused,
                &store.state().mempool.validated_operations.ops,
                current_branch,
                &protocol,
            );
            store.service().rpc().respond(*rpc_id, v);
        }
        Action::MempoolRecvDone(MempoolRecvDoneAction { address, .. }) => {
            if let Some(peer) = store.state().mempool.peer_state.get(address) {
                if !peer.requesting_full_content.is_empty() {
                    store.dispatch(MempoolGetOperationsAction { address: *address });
                } else {
                    // if this mempool doesn't introduce new operations, we have nothing to do
                }
            }
        }
        Action::MempoolGetOperations(MempoolGetOperationsAction { address }) => {
            let peer = match store.state().mempool.peer_state.get(address) {
                Some(peer) => peer,
                None => return,
            };
            let ops = peer.requesting_full_content.iter().cloned().collect();
            store.dispatch(MempoolMarkOperationsAsPendingAction { address: *address });
            store.dispatch(PeerMessageWriteInitAction {
                address: *address,
                message: Arc::new(GetOperationsMessage::new(ops).into()),
            });
        }
        Action::MempoolOperationRecvDone(MempoolOperationRecvDoneAction { operation })
        | Action::MempoolOperationInject(MempoolOperationInjectAction { operation, .. }) => {
            if store.state().mempool.is_old_endorsement(operation) {
                return;
            }
            store.dispatch(MempoolValidateStartAction {
                operation: operation.clone(),
            });
        }
        Action::MempoolValidateStart(MempoolValidateStartAction { operation }) => {
            store.dispatch(ProtocolValidateOperationStartAction {
                operation: operation.clone(),
            });
        }
        Action::MempoolBroadcast(MempoolBroadcastAction { send_operations }) => {
            let addresses = store.state().peers.iter_addr().cloned().collect::<Vec<_>>();
            for address in addresses {
                store.dispatch(MempoolSendAction {
                    address,
                    send_operations: *send_operations,
                    requested_explicitly: false,
                });
            }
        }
        Action::MempoolAskCurrentHead(MempoolAskCurrentHeadAction {}) => {
            let addresses = store.state().peers.iter_addr().cloned().collect::<Vec<_>>();
            let message = GetCurrentHeadMessage::new(store.state().config.chain_id.clone());
            let message = Arc::new(PeerMessageResponse::from(message));
            for address in addresses {
                store.dispatch(PeerMessageWriteInitAction {
                    address: address.clone(),
                    message: message.clone(),
                });
            }
        }
        Action::MempoolSendValidated(MempoolSendValidatedAction { address }) => {
            let head_state = match store.state().mempool.local_head_state.clone() {
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

            let known_valid = peer.known_valid_to_send.clone();
            let message = CurrentHeadMessage::new(
                store.state().config.chain_id.clone(),
                head_state.header.clone(),
                Mempool::new(known_valid.clone(), vec![]),
            );
            let message = Arc::new(PeerMessageResponse::from(message));

            store.dispatch(PeerMessageWriteInitAction {
                address: address.clone(),
                message,
            });
            store.dispatch(MempoolBroadcastDoneAction {
                address: address.clone(),
                pending: vec![],
                known_valid,
                cleanup_known_valid: true,
            });
        }
        Action::MempoolSend(MempoolSendAction {
            address,
            send_operations,
            requested_explicitly,
        }) => {
            let head_state = match store.state().mempool.local_head_state.clone() {
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

            // TODO(vlad): for debug
            let debug = false;
            let known_valid = if *requested_explicitly {
                if debug {
                    vec![]
                } else {
                    let delayed_endorsements = store
                        .state()
                        .mempool
                        .validated_operations
                        .branch_delayed
                        .iter()
                        .filter_map(|v| {
                            if v.is_endorsement? && !peer.seen_operations.contains(&v.hash) {
                                Some(v.hash.clone())
                            } else {
                                None
                            }
                        });
                    store
                        .state()
                        .mempool
                        .validated_operations
                        .applied
                        .iter()
                        .filter_map(|v| {
                            if !peer.seen_operations.contains(&v.hash) {
                                Some(v.hash.clone())
                            } else {
                                None
                            }
                        })
                        .chain(delayed_endorsements)
                        .collect::<Vec<_>>()
                }
            } else {
                store
                    .state()
                    .mempool
                    .validated_operations
                    .ops
                    .iter()
                    .filter_map(|(hash, _)| {
                        if !peer.seen_operations.contains(&hash.0) {
                            Some(hash.0.clone())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
            };
            // TODO(vlad):
            let pending = if *requested_explicitly && !debug {
                store
                    .state()
                    .mempool
                    .pending_operations
                    .iter()
                    .filter(|(hash, op)| {
                        !peer.seen_operations.contains(&hash.0) && head_state.hash.eq(op.branch())
                    })
                    .map(|(hash, _)| hash.0.clone())
                    .collect::<Vec<_>>()
            } else {
                vec![]
            };
            let mempool = if *send_operations {
                let mempool = Mempool::new(known_valid.clone(), pending.clone());
                if mempool.is_empty() {
                    return;
                }
                mempool
            } else {
                Mempool::default()
            };
            let message = CurrentHeadMessage::new(
                store.state().config.chain_id.clone(),
                head_state.header.clone(),
                mempool,
            );
            let message = Arc::new(PeerMessageResponse::from(message));

            store.dispatch(PeerMessageWriteInitAction {
                address: address.clone(),
                message,
            });
            store.dispatch(MempoolBroadcastDoneAction {
                address: address.clone(),
                pending,
                known_valid,
                cleanup_known_valid: false,
            });
        }
        _ => (),
    }
}
