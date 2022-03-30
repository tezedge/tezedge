// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use redux_rs::Store;
use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

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

use crate::{block_applier::BlockApplierApplyState, current_head_precheck::CurrentHeadState};
use crate::{
    current_head_precheck::CurrentHeadPrecheckSuccessAction,
    peer::message::{read::PeerMessageReadSuccessAction, write::PeerMessageWriteInitAction},
    prechecker::prechecker_actions::{PrecheckerApplied, PrecheckerErrored},
};
use crate::{
    mempool::mempool_state::OperationState,
    prechecker::prechecker_actions::{
        PrecheckerPrecacheEndorsingRightsAction, PrecheckerPrecheckOperationRequestAction,
        PrecheckerPrecheckOperationResponse, PrecheckerPrecheckOperationResponseAction,
        PrecheckerSetNextBlockProtocolAction,
    },
    rights::Slot,
};
use crate::{
    peer::remote_requests::current_branch_get::PeerRemoteRequestsCurrentBranchGetInitAction,
    protocol::ProtocolAction,
};
use crate::{
    service::{PrevalidatorService, RpcService},
    Action, ActionWithMeta, Service, State,
};

use super::{
    mempool_actions::MempoolRpcEndorsementsStatusGetAction,
    mempool_actions::*,
    monitored_operation::{MempoolOperations, MonitoredOperation},
};

fn chain_name(state: &State) -> &str {
    state.config.shell_compatibility_version.chain_name()
}

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
        Action::MempoolFlush(MempoolFlushAction {}) => {
            let mempool_state = &store.state().mempool;
            let ops = mempool_state
                .pending_operations
                .iter()
                .map(|(_, op)| op.clone())
                .collect::<Vec<_>>();
            for operation in ops {
                store.dispatch(MempoolValidateStartAction { operation });
            }
        }
        Action::Protocol(act) => {
            match act {
                ProtocolAction::PrevalidatorReady(_) => {
                    store.dispatch(MempoolFlushAction {});
                }
                ProtocolAction::OperationValidated(response) => {
                    let addresses = store.state().peers.iter_addr().cloned().collect::<Vec<_>>();

                    for address in addresses {
                        let state = store.state();
                        let applied_iter = response.result.applied.iter();
                        let applied_to_send_iter = applied_iter.map(|v| &v.hash);

                        let branch_delayed_iter = response.result.branch_delayed.iter();
                        let consensus_ops_branch_delayed_iter = branch_delayed_iter
                            .filter(|v| v.is_endorsement.unwrap_or(false))
                            .map(|v| &v.hash);

                        let known_valid = applied_to_send_iter
                            .chain(consensus_ops_branch_delayed_iter)
                            .filter(|hash| !state.mempool.has_peer_seen_op(address, hash))
                            .cloned()
                            .collect();

                        store.dispatch(MempoolSendValidatedAction {
                            address,
                            known_valid,
                        });
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
                        // `ProtocolAction::OperationValidated` action can happens only
                        // if we have a prevalidator
                        let prevalidator = match store.state().mempool.prevalidator.as_ref() {
                            Some(v) => v,
                            None => return,
                        };
                        let prot = prevalidator.protocol.to_base58_check();
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
                        let outdated = if stream.outdated {
                            response.result.outdated.as_slice()
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
                            .chain(MonitoredOperation::collect_errored(outdated, ops, &prot))
                            .collect::<Vec<_>>();
                        if let Ok(json) = serde_json::to_value(resp) {
                            store
                                .service()
                                .rpc()
                                .respond_stream(stream.rpc_id, Some(json));
                        }
                    }
                }
                _ => {}
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
                PeerMessage::CurrentHead(ref current_head) => {
                    if store.state().mempool.running_since.is_none() {
                        return;
                    }
                    if current_head.chain_id().ne(&store.state().config.chain_id) {
                        return;
                    }

                    let block_hash = match current_head.current_block_header().message_typed_hash()
                    {
                        Ok(hash) => hash,
                        Err(_) => return,
                    };
                    let message = current_head.current_mempool().clone();
                    store.dispatch(MempoolRecvDoneAction {
                        address: *address,
                        block_hash,
                        prev_block_hash: current_head.current_block_header().predecessor().clone(),
                        message,
                        level: current_head.current_block_header().level(),
                        timestamp: current_head.current_block_header().timestamp().into(),
                        proto: current_head.current_block_header().proto(),
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
        Action::BlockApplierApplySuccess(_) => {
            let chain_id = store.state().config.chain_id.clone();
            let (block, apply_result) = match &store.state().block_applier.current {
                BlockApplierApplyState::Success {
                    block,
                    apply_result,
                    ..
                } => (block, apply_result),
                _ => return,
            };

            if let Some(local_head_state) = store.state().mempool.local_head_state.as_ref() {
                if local_head_state.hash != block.hash {
                    let req = BeginConstructionRequest {
                        chain_id,
                        predecessor: local_head_state.header.clone(),
                        protocol_data: None,
                        predecessor_block_metadata_hash: local_head_state.metadata_hash.clone(),
                        predecessor_ops_metadata_hash: local_head_state.ops_metadata_hash.clone(),
                    };
                    store
                        .service()
                        .prevalidator()
                        .begin_construction_for_prevalidation(req);
                    return;
                }
            }

            if store.state().mempool.running_since.is_some() {
                let req = BeginConstructionRequest {
                    chain_id,
                    predecessor: (*block.header).clone(),
                    protocol_data: None,
                    predecessor_block_metadata_hash: apply_result.block_metadata_hash.clone(),
                    predecessor_ops_metadata_hash: apply_result.ops_metadata_hash.clone(),
                };
                store
                    .service()
                    .prevalidator()
                    .begin_construction_for_prevalidation(req);
                store.dispatch(MempoolBroadcastAction {
                    send_operations: false,
                    prechecked_head: None,
                });
            }
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
        Action::MempoolRegisterOperationsStream(act) => {
            // TODO(vlad): duplicated code
            let ops = &store.state().mempool.validated_operations.ops;
            let refused_ops = &store.state().mempool.validated_operations.refused_ops;
            let prot = match &store.state().mempool.prevalidator {
                Some(prevalidator) => prevalidator.protocol.to_base58_check(),
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
            let outdated = if act.outdated {
                store
                    .state()
                    .mempool
                    .validated_operations
                    .outdated
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
                .chain(MonitoredOperation::collect_errored(outdated, ops, &prot))
                .collect::<Vec<_>>();
            if let Ok(json) = serde_json::to_value(&resp) {
                slog::trace!(&store.state().log, "============\n{:#?}", resp);
                store.service().rpc().respond_stream(act.rpc_id, Some(json));
            }
        }
        Action::MempoolGetPendingOperations(MempoolGetPendingOperationsAction { rpc_id }) => {
            let empty = MempoolOperations::default();
            let prevalidator = match &store.state().mempool.prevalidator {
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
            prev_block_hash,
            proto,
            level,
            ..
        }) => {
            // Ask prechecker to precache endorsing rights for new level, using latest applied block
            if store.state().mempool.first_current_head
                && crate::prechecker::prechecking_enabled(store.state(), prev_block_hash)
            {
                if let Some(current_head) = store
                    .state
                    .get()
                    .mempool
                    .local_head_state
                    .as_ref()
                    .map(|lhs| lhs.hash.clone())
                {
                    store.dispatch(PrecheckerPrecacheEndorsingRightsAction {
                        current_head,
                        level: *level,
                    });
                }
                store.dispatch(PrecheckerSetNextBlockProtocolAction { proto: *proto });
            }
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
        Action::MempoolOperationRecvDone(MempoolOperationRecvDoneAction { operation })
            if store
                .state()
                .mempool
                .is_old_consensus_operation(chain_name(store.state()), operation) =>
        {

            // TODO store rejected endorsement for diagnosis?
        }
        Action::MempoolOperationInject(MempoolOperationInjectAction {
            operation, rpc_id, ..
        }) if store
            .state()
            .mempool
            .is_old_consensus_operation(chain_name(store.state()), operation) =>
        {
            let current_head = store
                .state
                .get()
                .mempool
                .local_head_state
                .as_ref()
                .as_ref()
                .map(|v| &v.hash);
            store.service.rpc().respond(
                *rpc_id,
                serde_json::json!({
                    "error": "Endorsement branch is too old",
                    "endorsement_branch": operation.branch(),
                    "current_head": current_head,
                }),
            );
        }
        Action::MempoolOperationRecvDone(MempoolOperationRecvDoneAction { operation })
        | Action::MempoolOperationInject(MempoolOperationInjectAction { operation, .. }) => {
            if crate::prechecker::prechecking_enabled(store.state(), operation.branch()) {
                store.dispatch(PrecheckerPrecheckOperationRequestAction {
                    operation: operation.clone(),
                });
            } else {
                store.dispatch(MempoolValidateStartAction {
                    operation: operation.clone(),
                });
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

                    if let Some(prevalidator) = store.state().mempool.prevalidator.as_ref() {
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
                        let error = if let PrecheckerPrecheckOperationResponse::Refused(
                            PrecheckerErrored { error, .. },
                        ) = response
                        {
                            vec![error.clone()]
                        } else {
                            Vec::new()
                        };

                        let protocol = prevalidator.protocol.to_base58_check();
                        let monitored_operation = MonitoredOperation::new(
                            operation_decoded_contents.branch(),
                            protocol_data,
                            &protocol,
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
                }
                PrecheckerPrecheckOperationResponse::Prevalidate(prevalidate) => {
                    if let Some(operation) = store
                        .state
                        .get()
                        .mempool
                        .pending_operations
                        .get(&prevalidate.hash)
                        .cloned()
                    {
                        store.dispatch(MempoolValidateStartAction { operation });
                    } else {
                        // TODO
                    }
                }
                _ => (),
            }
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
                    .prevalidator()
                    .validate_operation_for_prevalidation(validate_req);
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
            let head_state = match store.state().mempool.local_head_state.clone() {
                Some(v) => v,
                None => {
                    // should always have current head here
                    // TODO(vlad): should be forbidden by enabling condition
                    return;
                }
            };

            let current_mempool = Mempool::new(known_valid.clone(), vec![]);
            if current_mempool.is_empty() {
                return;
            }
            let message = CurrentHeadMessage::new(
                store.state().config.chain_id.clone(),
                head_state.header,
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
            block_hash: _,
        }) => {
            #[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
            struct OpStatus {
                state: OperationState,
                broadcast: bool,
                slot: Slot,
                #[serde(flatten)]
                times: HashMap<String, i64>,
            }

            let status = store
                .state
                .get()
                .mempool
                .operations_state
                .iter()
                .filter_map(|(op, state)| {
                    if let Some(slot) = state.endorsement_slot() {
                        let mut times = state.times.clone();
                        if let Some(t) = times.get("received_contents_time").cloned() {
                            times.insert("received_time".to_string(), t);
                        }
                        let status = OpStatus {
                            state: state.state,
                            broadcast: state.broadcast,
                            slot,
                            times: state.times.clone(),
                        };
                        Some((op.clone(), status))
                    } else {
                        None
                    }
                })
                .collect::<BTreeMap<_, _>>();
            store.service.rpc().respond(*rpc_id, status);
        }
        _ => (),
    }
}
