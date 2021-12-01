// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use redux_rs::Store;
use std::{collections::BTreeMap, sync::Arc};

use tezos_messages::p2p::encoding::{
    current_head::CurrentHeadMessage,
    mempool::Mempool,
    operation::{GetOperationsMessage, OperationMessage},
    peer::{PeerMessage, PeerMessageResponse},
};

use tezos_api::ffi::{BeginConstructionRequest, ValidateOperationRequest};

use crate::protocol::ProtocolAction;
use crate::{
    peer::message::{read::PeerMessageReadSuccessAction, write::PeerMessageWriteInitAction},
    prechecker::{
        PrecheckerPrecheckOperationRequestAction, PrecheckerPrecheckOperationResponse,
        PrecheckerPrecheckOperationResponseAction,
    },
};
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
    },
    mempool_state::HeadState,
    MempoolRpcEndorsementsStatusGetAction,
};

pub fn mempool_effects<S>(store: &mut Store<State, S, Action>, action: &ActionWithMeta)
where
    S: Service,
{
    // println!("{:#?}", action);
    if store.state().config.disable_mempool {
        if let Action::MempoolOperationInject(MempoolOperationInjectAction { rpc_id, .. }) =
            &action.action
        {
            store.service().rpc().respond(
                rpc_id.clone(),
                serde_json::Value::String("mempool disabled".to_string()),
            );
        }
        return;
    }
    match &action.action {
        Action::Protocol(act) => {
            match act {
                ProtocolAction::PrevalidatorForMempoolReady(_) => {
                    let ops = store.state().mempool.wait_prevalidator_operations.clone();
                    for (_, operation) in ops {
                        store.dispatch(MempoolValidateStartAction { operation });
                    }
                }
                ProtocolAction::OperationValidated(_) => {
                    store.dispatch(MempoolBroadcastAction {});
                    // respond
                    let resp = if store.state().mempool.local_head_state.is_some() {
                        serde_json::Value::Null
                    } else {
                        serde_json::Value::String("head is not ready".to_string())
                    };
                    let to_respond = store
                        .state()
                        .mempool
                        .injected_rpc_ids
                        .values()
                        .cloned()
                        .collect::<Vec<_>>();
                    for rpc_id in to_respond {
                        store.service().rpc().respond(rpc_id, resp.clone());
                    }
                    store.dispatch(MempoolRpcRespondAction {});
                }
                _ => {}
            }
            // panic!("{:?}", act);
        }
        Action::PeerMessageReadSuccess(PeerMessageReadSuccessAction { message, address }) => {
            match message.message() {
                PeerMessage::CurrentHead(ref current_head) => {
                    let message = current_head.current_mempool().clone();
                    let head_state = HeadState {
                        chain_id: current_head.chain_id().clone(),
                        current_block: current_head.current_block_header().clone(),
                    };
                    store.dispatch(MempoolRecvDoneAction {
                        address: *address,
                        head_state,
                        message,
                    });
                }
                PeerMessage::Operation(ref op) => {
                    store.dispatch(MempoolOperationRecvDoneAction {
                        address: *address,
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
            is_bootstrapped,
        }) => {
            if *is_bootstrapped {
                let req = BeginConstructionRequest {
                    chain_id: chain_id.clone(),
                    predecessor: block.clone(),
                    protocol_data: None,
                };
                store
                    .service()
                    .protocol()
                    .begin_construction_for_mempool(req);
            }
        }
        Action::MempoolRecvDone(MempoolRecvDoneAction {
            address,
            head_state,
            ..
        }) => {
            let blocks = &store.state().mempool.applied_block;
            let level = head_state.current_block.level();
            let pred = head_state.current_block.predecessor();
            if !blocks.contains(pred) && level > 1 {
                // if predecessor of the head is not applied,
                // we did not bootstrapped yet, should not handle mempool
                return;
            }
            let req = BeginConstructionRequest {
                chain_id: head_state.chain_id.clone(),
                predecessor: head_state.current_block.clone(),
                protocol_data: None,
            };
            store
                .service()
                .protocol()
                .begin_construction_for_mempool(req);
            if let Some(peer) = store.state().mempool.peer_state.get(address) {
                if !peer.requesting_full_content.is_empty() {
                    store.dispatch(MempoolGetOperationsAction { address: *address });
                } else {
                    // if this mempool doesn't introduce new operations, we have nothing to do
                }
            }
        }
        Action::MempoolGetOperations(MempoolGetOperationsAction { address }) => {
            if let Some(peer) = store.state().mempool.peer_state.get(address) {
                let ops = peer.requesting_full_content.iter().cloned().collect();
                store.dispatch(MempoolGetOperationsPendingAction { address: *address });
                store.dispatch(PeerMessageWriteInitAction {
                    address: *address,
                    message: Arc::new(GetOperationsMessage::new(ops).into()),
                });
            }
        }
        Action::MempoolOperationRecvDone(MempoolOperationRecvDoneAction { operation, .. })
        | Action::MempoolOperationInject(MempoolOperationInjectAction { operation, .. }) => {
            store.dispatch(PrecheckerPrecheckOperationRequestAction {
                operation: operation.clone(),
            });
        }
        Action::PrecheckerPrecheckOperationResponse(
            PrecheckerPrecheckOperationResponseAction { response },
        ) => {
            match response {
                PrecheckerPrecheckOperationResponse::Applied(_)
                | PrecheckerPrecheckOperationResponse::Refused(_) => {
                    store.dispatch(MempoolBroadcastAction {});
                    // respond
                    let resp = if store.state().mempool.local_head_state.is_some() {
                        serde_json::Value::Null
                    } else {
                        serde_json::Value::String("head is not ready".to_string())
                    };
                    let to_respond = store
                        .state()
                        .mempool
                        .injected_rpc_ids
                        .values()
                        .cloned()
                        .collect::<Vec<_>>();
                    for rpc_id in to_respond {
                        store.service().rpc().respond(rpc_id, resp.clone());
                    }
                    store.dispatch(MempoolRpcRespondAction {});
                }
                PrecheckerPrecheckOperationResponse::Prevalidate(operation) => {
                    store.dispatch(MempoolValidateStartAction {
                        operation: operation.clone(),
                    });
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
                    .protocol()
                    .validate_operation_for_mempool(validate_req);
            } else {
                store.dispatch(MempoolValidateWaitPrevalidatorAction {
                    operation: operation.clone(),
                });
            }
        }
        Action::MempoolBroadcast(MempoolBroadcastAction {}) => {
            let (head_state, head_hash) = match store.state().mempool.local_head_state.clone() {
                Some(v) => v,
                None => {
                    // should always have current head here
                    // TODO(vlad): should be forbidden by enabling condition
                    return;
                }
            };
            let addresses = store.state().peers.iter_addr().cloned().collect::<Vec<_>>();
            // TODO(vlad): add action removing peer_state for disconnected peers
            for address in addresses {
                let peer = match store.state().mempool.peer_state.get(&address) {
                    Some(v) => v,
                    None => continue,
                };

                let known_valid = store
                    .state()
                    .mempool
                    .validated_operations
                    .ops
                    .iter()
                    .filter(|(hash, op)| {
                        !peer.seen_operations.contains(&hash.0) && head_hash.eq(op.branch())
                    })
                    .map(|(hash, _)| &hash.0)
                    .cloned()
                    .collect::<Vec<_>>();
                let pending = store
                    .state()
                    .mempool
                    .pending_operations
                    .iter()
                    .filter(|(hash, op)| {
                        !peer.seen_operations.contains(&hash.0) && head_hash.eq(op.branch())
                    })
                    .map(|(hash, _)| &hash.0)
                    .cloned()
                    .collect::<Vec<_>>();
                let message = CurrentHeadMessage::new(
                    head_state.chain_id.clone(),
                    head_state.current_block.clone(),
                    Mempool::new(known_valid.clone(), pending.clone()),
                );
                let message = Arc::new(PeerMessageResponse::from(message));

                store.dispatch(PeerMessageWriteInitAction {
                    address,
                    message: message.clone(),
                });
                store.dispatch(MempoolBroadcastDoneAction {
                    address,
                    pending,
                    known_valid,
                });
            }
        }

        Action::MempoolRpcEndorsementsStatusGet(MempoolRpcEndorsementsStatusGetAction {
            rpc_id,
            block_hash,
        }) => {
            if store.state.get().mempool.head_hash() == Some(block_hash) {
                let status = &store
                    .state
                    .get()
                    .mempool
                    .operations_state
                    .iter()
                    .filter_map(|(op, state)| {
                        if let Some(slot) = state.endorsement_slot() {
                            let mut json = match serde_json::to_value(state) {
                                Ok(v) => v,
                                Err(_) => return None,
                            };
                            let json_obj = json.as_object_mut()?;
                            let _ = json_obj.remove("protocol_data")?;
                            json_obj.insert("slot".to_string(), slot.clone());
                            Some((op, json))
                        } else {
                            None
                        }
                    })
                    .collect::<BTreeMap<_, _>>();
                store.service.rpc().respond(*rpc_id, status);
            } else {
                store
                    .service
                    .rpc()
                    .respond(*rpc_id, serde_json::json!({"error": "non-current block"}));
            }
        }
        _ => (),
    }
}
