// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crypto::hash::OperationHash;
use redux_rs::Store;
use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use tezos_messages::{
    p2p::{
        binary_message::MessageHash,
        encoding::{
            current_head::{CurrentHeadMessage, GetCurrentHeadMessage},
            mempool::Mempool,
            operation::{GetOperationsMessage, OperationMessage},
            peer::{PeerMessage, PeerMessageResponse},
        },
    },
    protocol::{proto_010, proto_011, proto_012},
};

use crate::{
    current_head_precheck::{CurrentHeadPrecheckSuccessAction, CurrentHeadState},
    mempool::mempool_state::OperationState,
    peer::message::{read::PeerMessageReadSuccessAction, write::PeerMessageWriteInitAction},
    prechecker::{
        prechecker_actions::{
            PrecheckerApplied, PrecheckerErrored, PrecheckerPrecheckBlockAction,
            PrecheckerPrecheckOperationRequestAction, PrecheckerPrecheckOperationResponse,
            PrecheckerPrecheckOperationResponseAction,
        },
        OperationDecodedContents,
    },
    rights::Slot,
    service::RpcService,
    Action, ActionWithMeta, Service, State,
};

use super::{
    mempool_actions::MempoolRpcEndorsementsStatusGetAction,
    mempool_actions::*,
    monitored_operation::{MempoolOperations, MonitoredOperation},
    validator::{MempoolValidatorInitAction, MempoolValidatorValidateInitAction},
    BroadcastState, MempoolOperation, OperationKind,
};

pub fn mempool_effects<S>(store: &mut Store<State, S, Action>, action: &ActionWithMeta)
where
    S: Service,
{
    // println!("{:#?}", action);
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
        Action::MempoolOperationValidateNext(_) => {
            let mempool = &store.state().mempool;

            // Find operation with highest priority.
            let (op_hash, op_content) = match mempool
                .injecting_rpc_ids
                .iter()
                .filter_map(|(op_hash, _)| {
                    mempool
                        .pending_operations
                        .get(op_hash)
                        .map(|content| (10, op_hash, content))
                })
                .chain(
                    mempool
                        .pending_operations
                        .iter()
                        .map(|(op_hash, op_content)| {
                            let priority = match OperationKind::from_operation_content_raw(
                                op_content.data().as_ref(),
                            ) {
                                OperationKind::Endorsement | OperationKind::EndorsementWithSlot => {
                                    9
                                }
                                OperationKind::Preendorsement => 8,
                                _ => 0,
                            };
                            (priority, op_hash, op_content)
                        }),
                )
                .max_by_key(|(priority, _, _)| *priority)
            {
                Some(v) => (v.1.clone(), v.2.clone()),
                None => return,
            };

            store.dispatch(MempoolValidatorValidateInitAction {
                op_hash,
                op_content,
            });
        }
        Action::MempoolValidatorReady(_) => {
            store.dispatch(MempoolOperationValidateNextAction {});
        }
        Action::MempoolValidatorValidateSuccess(content) => {
            if content.result.is_applied() {
                let addresses = store.state().peers.iter_addr().cloned().collect::<Vec<_>>();

                for address in addresses {
                    if store
                        .state()
                        .mempool
                        .has_peer_seen_op(address, &content.op_hash)
                    {
                        continue;
                    }

                    store.dispatch(MempoolSendValidatedAction {
                        address,
                        known_valid: vec![content.op_hash.clone()],
                    });
                }
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
            store.dispatch(MempoolOperationValidateNextAction {});
            let streams = store.state().mempool.operation_streams.clone();
            for stream in streams {
                let ops = &store.state().mempool.validated_operations.ops;
                // `PrevalidatorAction::OperationValidated` action can happens only
                // if we have a prevalidator
                let prevalidator = match store.state().mempool.validator.prevalidator() {
                    Some(v) => v,
                    None => return,
                };
                let prot = prevalidator.protocol.to_base58_check();
                let resp: Vec<_> = match content.result.as_result() {
                    Ok(applied) => {
                        MonitoredOperation::collect_applied([applied], ops, &prot).collect()
                    }
                    Err(errored) => {
                        MonitoredOperation::collect_errored([errored], ops, &prot).collect()
                    }
                };
                if resp.is_empty() {
                    return;
                }
                if let Ok(json) = serde_json::to_value(resp) {
                    store
                        .service()
                        .rpc()
                        .respond_stream(stream.rpc_id, Some(json));
                }
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
                        prechecked_head: None,
                    });
                }
                PeerMessage::Operation(ref op) => {
                    let hash = match op.message_typed_hash() {
                        Ok(v) => v,
                        Err(err) => {
                            // TODO(vlad): peer send bad operation, should log the error,
                            // maybe should disconnect the peer
                            let _ = err;
                            return;
                        }
                    };
                    store.dispatch(MempoolOperationRecvDoneAction {
                        hash,
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
        Action::BlockApplierApplySuccess(_) => {
            // close streams
            let streams = store.state().mempool.operation_streams.clone();
            store.dispatch(MempoolUnregisterOperationsStreamsAction {});
            for stream in streams {
                store
                    .service()
                    .rpc()
                    .respond_stream(stream.rpc_id, Some(serde_json::json!([])));
                store.service().rpc().respond_stream(stream.rpc_id, None);
            }
        }
        Action::ProtocolRunnerReady(_) => {
            if store.state().mempool.running_since.is_some() {
                store.dispatch(MempoolValidatorInitAction {});
            }
        }
        Action::CurrentHeadRehydrated(_) | Action::CurrentHeadUpdate(_) => {
            if store.state().mempool.running_since.is_some() {
                store.dispatch(MempoolBroadcastAction {
                    send_operations: false,
                    prechecked_head: None,
                });
                store.dispatch(MempoolValidatorInitAction {});
            }
        }
        Action::MempoolRegisterOperationsStream(act) => {
            // TODO(vlad): duplicated code
            let ops = &store.state().mempool.validated_operations.ops;
            let prot = match &store.state().mempool.validator.prevalidator() {
                Some(prevalidator) => prevalidator.protocol.to_base58_check(),
                None => return,
            };
            let validated_operations = &store.state().mempool.validated_operations;
            let applied = validated_operations
                .applied
                .iter()
                .take_while(|_| act.applied);
            let refused = validated_operations
                .refused
                .iter()
                .take_while(|_| act.refused);
            let branch_delayed = validated_operations
                .branch_delayed
                .iter()
                .take_while(|_| act.branch_delayed);
            let branch_refused = validated_operations
                .branch_refused
                .iter()
                .take_while(|_| act.branch_refused);
            let outdated = validated_operations
                .outdated
                .iter()
                .take_while(|_| act.outdated);
            let resp = std::iter::empty()
                .chain(MonitoredOperation::collect_applied(applied, ops, &prot))
                .chain(MonitoredOperation::collect_errored(refused, ops, &prot))
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
                .chain(MonitoredOperation::collect_errored(outdated, ops, &prot))
                .collect::<Vec<_>>();
            if let Ok(json) = serde_json::to_value(&resp) {
                store.service().rpc().respond_stream(act.rpc_id, Some(json));
            }
        }
        Action::MempoolGetPendingOperations(MempoolGetPendingOperationsAction { rpc_id }) => {
            let empty = MempoolOperations::default();
            let prevalidator = match store.state().mempool.validator.prevalidator() {
                Some(v) => v,
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
                &v_ops.outdated,
                &v_ops.ops,
                &prevalidator.protocol,
            );
            store.service().rpc().respond(*rpc_id, v);
        }
        Action::MempoolRecvDone(MempoolRecvDoneAction {
            address,
            block_hash,
            block_header,
            ..
        }) => {
            store.dispatch(PrecheckerPrecheckBlockAction {
                block_hash: block_hash.clone(),
                block_header: block_header.clone(),
            });
            // TODO enable this when rights calculation is implemented
            // let prev_block_hash = block_header.predecessor();
            // let level = block_header.level();
            // Ask prechecker to precache endorsing rights for new level, using latest applied block
            // if crate::prechecker::prechecking_enabled(store.state(), prev_block_hash) {
            //     if let Some(current_head) = store
            //         .state
            //         .get()
            //         .mempool
            //         .local_head_state
            //         .as_ref()
            //         .map(|lhs| lhs.hash.clone())
            //     {
            //         store.dispatch(PrecheckerPrecacheEndorsingRightsAction {
            //             current_head,
            //             level,
            //         });
            //     }
            // }
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
            for message in GetOperationsMessage::from_operations(ops) {
                store.dispatch(PeerMessageWriteInitAction {
                    address: *address,
                    message: Arc::new(message.into()),
                });
            }
        }
        Action::MempoolOperationRecvDone(MempoolOperationRecvDoneAction { hash, operation })
        | Action::MempoolOperationInject(MempoolOperationInjectAction {
            hash, operation, ..
        }) => {
            if store.state().mempool.prechecking_operations.contains(hash) {
                store.dispatch(PrecheckerPrecheckOperationRequestAction {
                    operation: operation.clone(),
                });
            } else {
                store.dispatch(MempoolOperationValidateNextAction {});
            }
        }
        Action::PrecheckerPrecheckOperationResponse(
            PrecheckerPrecheckOperationResponseAction { response },
        ) => {
            match response {
                PrecheckerPrecheckOperationResponse::Applied(PrecheckerApplied {
                    operation_decoded_contents,
                    hash,
                    ..
                })
                | PrecheckerPrecheckOperationResponse::Refused(PrecheckerErrored {
                    operation_decoded_contents,
                    hash,
                    ..
                }) => {
                    store.dispatch(MempoolBroadcastAction {
                        send_operations: true,
                        prechecked_head: Some(operation_decoded_contents.branch().clone()),
                    });

                    // respond to injection RPC
                    let resp = if store.state().mempool.local_head_state.is_some() {
                        serde_json::Value::Null
                    } else {
                        serde_json::Value::String("head is not ready".to_string())
                    };
                    let to_respond = store.state().mempool.injected_rpc_ids.clone();
                    for rpc_id in to_respond {
                        store.service().rpc().respond(rpc_id, resp.clone());
                    }
                    store.dispatch(MempoolRpcRespondAction {});

                    // respond to mempool_operations
                    let streams = store.state().mempool.operation_streams.clone();
                    if streams.is_empty() {
                        return;
                    }

                    let protocol = match operation_decoded_contents {
                        OperationDecodedContents::Proto010(_) => proto_010::PROTOCOL_HASH,
                        OperationDecodedContents::Proto011(_) => proto_011::PROTOCOL_HASH,
                        OperationDecodedContents::Proto012(_) => proto_012::PROTOCOL_HASH,
                    };

                    let (protocol_data, protocol_data_parse_error) = if let Some(json_object) =
                        operation_decoded_contents.as_json().as_object()
                    {
                        (
                            json_object
                                .iter()
                                .map(|(k, v)| (k.clone(), v.clone()))
                                .collect::<HashMap<_, _>>(),
                            Option::<String>::None,
                        )
                    } else {
                        (
                            HashMap::new(),
                            Some("Cannot interpred protocol data as object".to_string()),
                        )
                    };
                    let error =
                        if let PrecheckerPrecheckOperationResponse::Refused(PrecheckerErrored {
                            error,
                            ..
                        }) = response
                        {
                            vec![error.clone()]
                        } else {
                            Vec::new()
                        };

                    let monitored_operation = MonitoredOperation::new(
                        operation_decoded_contents.branch(),
                        protocol_data,
                        protocol,
                        hash,
                        error,
                        protocol_data_parse_error,
                    );

                    if let Ok(json) = serde_json::to_value(vec![monitored_operation]) {
                        for stream in streams {
                            store
                                .service()
                                .rpc()
                                .respond_stream(stream.rpc_id, Some(json.clone()));
                        }
                    }
                }
                PrecheckerPrecheckOperationResponse::Prevalidate(_) => {
                    store.dispatch(MempoolOperationValidateNextAction {});
                }
                _ => (),
            }
        }
        Action::MempoolBroadcast(MempoolBroadcastAction {
            send_operations,
            prechecked_head,
        }) => {
            let addresses = store
                .state()
                .peers
                .iter_handshaked()
                .map(|(a, _)| *a)
                .collect::<Vec<_>>();
            for address in addresses {
                store.dispatch(MempoolSendAction {
                    address,
                    send_operations: *send_operations,
                    requested_explicitly: false,
                    prechecked_head: prechecked_head.clone(),
                });
            }
        }
        Action::MempoolAskCurrentHead(MempoolAskCurrentHeadAction {}) => {
            let addresses = store.state().peers.iter_addr().cloned().collect::<Vec<_>>();
            let message = GetCurrentHeadMessage::new(store.state().config.chain_id.clone());
            let message = Arc::new(PeerMessageResponse::from(message));
            for address in addresses {
                store.dispatch(PeerMessageWriteInitAction {
                    address,
                    message: message.clone(),
                });
            }
        }
        Action::MempoolSendValidated(MempoolSendValidatedAction {
            address,
            known_valid,
        }) => {
            let current_head_header = match store.state().current_head.get() {
                Some(v) => (*v.header).clone(),
                None => return,
            };

            let current_mempool = Mempool::new(known_valid.clone(), vec![]);
            if current_mempool.is_empty() {
                return;
            }
            let message = CurrentHeadMessage::new(
                store.state().config.chain_id.clone(),
                current_head_header,
                current_mempool,
            );
            let message = Arc::new(PeerMessageResponse::from(message));

            store.dispatch(PeerMessageWriteInitAction {
                address: *address,
                message,
            });
            store.dispatch(MempoolBroadcastDoneAction {
                address: *address,
                pending: vec![],
                known_valid: known_valid.clone(),
            });
        }
        Action::MempoolSend(MempoolSendAction {
            address,
            send_operations,
            requested_explicitly,
            prechecked_head,
        }) => {
            let applied_block = match store.state().current_head.get() {
                Some(v) => v,
                None => return,
            };
            let (block_is_applied, header, head_hash) = match prechecked_head.as_ref() {
                Some(prechecked_head) if prechecked_head != &applied_block.hash => {
                    if let Some(CurrentHeadState::Prechecked { block_header, .. }) =
                        store.state().current_heads.candidates.get(prechecked_head)
                    {
                        (false, block_header, prechecked_head)
                    } else {
                        return;
                    }
                }
                _ => (true, &*applied_block.header, &applied_block.hash),
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
                            if v.is_endorsement? {
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
                        .map(|v| v.hash.clone())
                        .chain(delayed_endorsements)
                        .collect::<Vec<_>>()
                }
            } else {
                let seen_operations_default = Default::default();
                let seen_operations = match store.state().mempool.peer_state.get(address) {
                    Some(v) => &v.seen_operations,
                    None => &seen_operations_default,
                };
                store
                    .state()
                    .mempool
                    .validated_operations
                    .ops
                    .iter()
                    .filter_map(|(hash, op)| {
                        if !seen_operations.contains(hash)
                            // when broadcasting prechecked head, only include operations for that head
                            && (block_is_applied || head_hash == op.branch())
                        {
                            Some(hash.clone())
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
                    .filter_map(|(hash, op)| {
                        if head_hash.eq(op.branch()) {
                            Some(hash.clone())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
            } else {
                vec![]
            };
            let mempool = if *send_operations {
                let mempool = Mempool::new(known_valid.clone(), pending.clone());
                if !requested_explicitly && mempool.is_empty() {
                    return;
                }
                mempool
            } else {
                Mempool::default()
            };
            let message = CurrentHeadMessage::new(
                store.state().config.chain_id.clone(),
                header.clone(),
                mempool,
            );
            let message = Arc::new(PeerMessageResponse::from(message));
            store.dispatch(PeerMessageWriteInitAction {
                address: *address,
                message,
            });
            store.dispatch(MempoolBroadcastDoneAction {
                address: *address,
                pending,
                known_valid,
            });
        }
        Action::CurrentHeadPrecheckSuccess(CurrentHeadPrecheckSuccessAction {
            block_hash,
            injected,
            ..
        }) if !injected && !store.state().config.disable_block_precheck => {
            store.dispatch(MempoolBroadcastAction {
                send_operations: false,
                prechecked_head: Some(block_hash.clone()),
            });
        }

        Action::MempoolRpcEndorsementsStatusGet(MempoolRpcEndorsementsStatusGetAction {
            rpc_id,
            matcher,
        }) => {
            let status = collect_operations(&store.state().mempool.operations_state, matcher);
            store.service.rpc().respond(*rpc_id, status);
        }
        _ => (),
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
struct OpStatus {
    state: OperationState,
    broadcast: BroadcastState,
    slot: Option<Slot>,
    #[serde(flatten)]
    operation: Option<serde_json::Value>,
    #[serde(flatten)]
    times: HashMap<String, u64>,
}

fn collect_operations<M>(
    operations_state: &BTreeMap<OperationHash, MempoolOperation>,
    matcher: &M,
) -> BTreeMap<OperationHash, OpStatus>
where
    M: MempoolOperationMatcher,
{
    operations_state
        .iter()
        .filter(|(_, state)| matcher.matches(state))
        .map(|(op, state)| {
            let mut times = state.times.clone();
            if let Some(t) = times.get("received_contents_time").cloned() {
                times.insert("received_time".to_string(), t);
            }
            let status = OpStatus {
                state: state.state,
                broadcast: state.broadcast,
                slot: state.endorsement_slot(),
                operation: state.as_json(),
                times: state.times.clone(),
            };
            (op.clone(), status)
        })
        .collect::<BTreeMap<_, _>>()
}
