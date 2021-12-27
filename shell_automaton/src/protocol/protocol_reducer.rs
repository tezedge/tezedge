// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use tezos_messages::p2p::encoding::operation::Operation;

use crate::{Action, ActionWithMeta, State};

use super::protocol_state::OperationValidationState;

pub fn protocol_reducer(state: &mut State, action: &ActionWithMeta) {
    enum QueueSide {
        Front,
        Back,
    }

    fn enqueue(state: &mut State, side: QueueSide, operation: Operation) {
        let queue = match operation.data().first() {
            None => return, // the data of the operation is empty, should not happen
            Some(0x00) => &mut state.protocol.endorsements_to_validate,
            Some(_) => &mut state.protocol.operations_to_validate,
        };
        match side {
            QueueSide::Front => queue.push_front(operation),
            QueueSide::Back => queue.push_back(operation),
        }
    }

    fn dequeue(state: &mut State) -> Option<Operation> {
        if state.protocol.endorsements_to_validate.is_empty() {
            state.protocol.operations_to_validate.pop_front()
        } else {
            state.protocol.endorsements_to_validate.pop_front()
        }
    }

    match &action.action {
        Action::ProtocolConstructStatefulPrevalidatorStart(_)
        | Action::ProtocolConstructStatelessPrevalidatorStart(_) => {
            let new = match &state.protocol.operation_validation_state {
                OperationValidationState::Initial => OperationValidationState::Initial,
                OperationValidationState::WaitingOperations { prevalidator, .. } => {
                    OperationValidationState::WaitingPrevalidator {
                        protocol: prevalidator.protocol.clone(),
                    }
                }
                OperationValidationState::WaitingPrevalidator { protocol } => {
                    // unreachable
                    OperationValidationState::WaitingPrevalidator {
                        protocol: protocol.clone(),
                    }
                }
                OperationValidationState::ValidatingOperation {
                    prevalidator,
                    operation,
                    ..
                } => {
                    let protocol = prevalidator.protocol.clone();
                    let operation = operation.clone();
                    enqueue(state, QueueSide::Front, operation);
                    OperationValidationState::WaitingPrevalidator { protocol }
                }
            };
            state.protocol.operation_validation_state = new;
        }
        Action::ProtocolConstructStatefulPrevalidatorDone(action) => {
            if let Some(operation) = dequeue(state) {
                state.protocol.operation_validation_state =
                    OperationValidationState::ValidatingOperation {
                        prevalidator: action.prevalidator.clone(),
                        operation,
                        stateful: true,
                    };
            } else {
                state.protocol.operation_validation_state =
                    OperationValidationState::WaitingOperations {
                        prevalidator: action.prevalidator.clone(),
                        stateful: true,
                    };
            }
        }
        Action::ProtocolConstructStatelessPrevalidatorDone(action) => {
            if let Some(operation) = dequeue(state) {
                state.protocol.operation_validation_state =
                    OperationValidationState::ValidatingOperation {
                        prevalidator: action.prevalidator.clone(),
                        operation,
                        stateful: false,
                    };
            } else {
                state.protocol.operation_validation_state =
                    OperationValidationState::WaitingOperations {
                        prevalidator: action.prevalidator.clone(),
                        stateful: false,
                    };
            }
        }
        Action::ProtocolValidateOperationStart(action) => {
            let operation = action.operation.clone();
            match &state.protocol.operation_validation_state {
                OperationValidationState::WaitingOperations {
                    stateful,
                    prevalidator,
                } => {
                    state.protocol.operation_validation_state =
                        OperationValidationState::ValidatingOperation {
                            prevalidator: prevalidator.clone(),
                            operation,
                            stateful: *stateful,
                        };
                }
                _ => enqueue(state, QueueSide::Back, action.operation.clone()),
            }
        }
        Action::ProtocolValidateOperationDone(action) => {
            let stateful = match &state.protocol.operation_validation_state {
                OperationValidationState::ValidatingOperation { stateful, .. } => *stateful,
                _ => return, // the action is not allowed in such state
            };
            if let Some(operation) = dequeue(state) {
                state.protocol.operation_validation_state =
                    OperationValidationState::ValidatingOperation {
                        prevalidator: action.response.prevalidator.clone(),
                        operation,
                        stateful,
                    };
            } else {
                state.protocol.operation_validation_state =
                    OperationValidationState::WaitingOperations {
                        prevalidator: action.response.prevalidator.clone(),
                        stateful,
                    };
            }
        }
        _ => {}
    }
}
