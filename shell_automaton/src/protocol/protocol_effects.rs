// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::{Action, ActionWithMeta, Service, State, Store};

use super::protocol_actions::*;
use super::protocol_state::OperationValidationState;
use crate::service::{ProtocolError, ProtocolResponse, ProtocolService};

use tezos_api::ffi::ValidateOperationRequest;

pub fn protocol_effects<S>(store: &mut Store<S>, action: &ActionWithMeta)
where
    S: Service,
{
    fn is_queue_empty(state: &State) -> bool {
        state.protocol.endorsements_to_validate.is_empty()
            && state.protocol.operations_to_validate.is_empty()
    }

    match &action.action {
        Action::WakeupEvent(_) => {
            // what is the good bound for this loop?
            const BOUND: usize = 1024;
            let mut cnt = 0;
            loop {
                match store.service.protocol().try_recv() {
                    Err(ProtocolError::Empty) => break,
                    Err(ProtocolError::Disconnected) => break, // TODO: log error
                    Err(ProtocolError::Internal(_)) => (),     // TODO: log error
                    Ok(ProtocolResponse::PrevalidatorForMempoolReady(prevalidator)) => {
                        store.dispatch(ProtocolConstructStatefulPrevalidatorDoneAction {
                            prevalidator,
                        });
                    }
                    Ok(ProtocolResponse::PrevalidatorReady(prevalidator)) => {
                        store.dispatch(ProtocolConstructStatelessPrevalidatorDoneAction {
                            prevalidator,
                        });
                    }
                    Ok(ProtocolResponse::OperationValidated(response)) => {
                        store.dispatch(ProtocolValidateOperationDoneAction { response });
                    }
                }
                if cnt == BOUND {
                    break;
                }
                cnt += 1;
            }
        }
        Action::ProtocolConstructStatefulPrevalidatorStart(action) => {
            // here is asynchronous call to the protocol runner,
            // the response will be dispatched in `Action::WakeupEvent`
            // in `ProtocolResponse::PrevalidatorForMempoolReady`
            store
                .service()
                .protocol()
                .begin_construction_for_mempool(action.request.clone());
        }
        Action::ProtocolConstructStatelessPrevalidatorStart(action) => {
            // here is asynchronous call to the protocol runner,
            // the response will be dispatched in `Action::WakeupEvent`
            // in `ProtocolResponse::PrevalidatorReady`
            store
                .service()
                .protocol()
                .begin_construction_for_prevalidation(action.request.clone());
        }
        Action::ProtocolConstructStatefulPrevalidatorDone(_)
        | Action::ProtocolConstructStatelessPrevalidatorDone(_)
        | Action::ProtocolValidateOperationDone(_) => {
            match &store.state().protocol.operation_validation_state {
                OperationValidationState::ValidatingOperation {
                    prevalidator,
                    operation,
                    stateful,
                } => {
                    let request = ValidateOperationRequest {
                        prevalidator: prevalidator.clone(),
                        operation: operation.clone(),
                    };
                    // here is asynchronous call to the protocol runner,
                    // the response will be dispatched in `Action::WakeupEvent`
                    // in `ProtocolResponse::OperationValidated`
                    if *stateful {
                        store
                            .service()
                            .protocol()
                            .validate_operation_for_mempool(request)
                    } else {
                        store
                            .service()
                            .protocol()
                            .validate_operation_for_prevalidation(request)
                    }
                }
                _ => (),
            }
        }
        Action::ProtocolValidateOperationStart(_) if is_queue_empty(store.state()) => {
            match &store.state().protocol.operation_validation_state {
                OperationValidationState::ValidatingOperation {
                    prevalidator,
                    operation,
                    stateful,
                } => {
                    let request = ValidateOperationRequest {
                        prevalidator: prevalidator.clone(),
                        operation: operation.clone(),
                    };
                    // here is asynchronous call to the protocol runner,
                    // the response will be dispatched in `Action::WakeupEvent`
                    // in `ProtocolResponse::OperationValidated`
                    if *stateful {
                        store
                            .service()
                            .protocol()
                            .validate_operation_for_mempool(request)
                    } else {
                        store
                            .service()
                            .protocol()
                            .validate_operation_for_prevalidation(request)
                    }
                }
                _ => (),
            }
        }
        _ => (),
    }
}
