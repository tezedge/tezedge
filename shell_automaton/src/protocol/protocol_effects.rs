// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::{Action, ActionWithMeta, Service, State, Store};

use super::protocol_actions::*;
use super::protocol_state::ValidationState;
use crate::service::{ProtocolError, ProtocolResponse, ProtocolService};

use tezos_api::ffi::ValidateOperationRequest;

pub fn protocol_effects<S>(store: &mut Store<S>, action: &ActionWithMeta)
where
    S: Service,
{
    fn is_empty_queue(state: &State) -> bool {
        state.protocol.endorsements_to_validate.is_empty()
            && state.protocol.operations_to_validate.is_empty()
    }

    fn continue_validation<S>(store: &mut Store<S>)
    where
        S: Service,
    {
        match &store.state().protocol.validation_state {
            ValidationState::Validating {
                prevalidator,
                operation,
                stateful,
            } => {
                let request = ValidateOperationRequest {
                    prevalidator: prevalidator.clone(),
                    operation: operation.clone(),
                };
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

    // TODO(vlad): remove it
    // if matches!(
    //     &action.action,
    //     | Action::ProtocolConstructStatelessPrevalidatorStart(_)
    //     | Action::ProtocolConstructStatelessPrevalidatorDone(_)
    //     | Action::ProtocolValidateOperationStart(_)
    //     | Action::ProtocolValidateOperationDone(_)
    // ) {
    //     println!("{:?}", action);
    // }

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
                    Ok(ProtocolResponse::InitProtocolDone(_)) => (), // unused
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
            store
                .service()
                .protocol()
                .begin_construction_for_mempool(action.request.clone());
        }
        Action::ProtocolConstructStatelessPrevalidatorStart(action) => {
            store
                .service()
                .protocol()
                .begin_construction_for_prevalidation(action.request.clone());
        }
        Action::ProtocolConstructStatefulPrevalidatorDone(_)
        | Action::ProtocolConstructStatelessPrevalidatorDone(_) => continue_validation(store),
        Action::ProtocolValidateOperationStart(_) if is_empty_queue(store.state()) => {
            continue_validation(store)
        }
        Action::ProtocolValidateOperationDone(_) => continue_validation(store),
        _ => (),
    }
}
