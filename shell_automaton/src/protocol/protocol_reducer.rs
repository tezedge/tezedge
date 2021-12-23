// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use tezos_messages::p2p::encoding::operation::Operation;

use crate::{Action, ActionWithMeta, State};

use super::protocol_state::ValidationState;

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
            let new = match &state.protocol.validation_state {
                ValidationState::Initial => ValidationState::Initial,
                ValidationState::Ready { prevalidator, .. } => ValidationState::NotReady {
                    protocol: prevalidator.protocol.clone(),
                },
                ValidationState::NotReady { protocol } => {
                    // unreachable
                    ValidationState::NotReady {
                        protocol: protocol.clone(),
                    }
                }
                ValidationState::Validating {
                    prevalidator,
                    operation,
                    ..
                } => {
                    let protocol = prevalidator.protocol.clone();
                    let operation = operation.clone();
                    enqueue(state, QueueSide::Front, operation);
                    ValidationState::NotReady { protocol }
                }
            };
            state.protocol.validation_state = new;
        }
        Action::ProtocolConstructStatefulPrevalidatorDone(action) => {
            if let Some(operation) = dequeue(state) {
                state.protocol.validation_state = ValidationState::Validating {
                    prevalidator: action.prevalidator.clone(),
                    operation,
                    stateful: true,
                };
            } else {
                state.protocol.validation_state = ValidationState::Ready {
                    prevalidator: action.prevalidator.clone(),
                    stateful: true,
                };
            }
        }
        Action::ProtocolConstructStatelessPrevalidatorDone(action) => {
            if let Some(operation) = dequeue(state) {
                state.protocol.validation_state = ValidationState::Validating {
                    prevalidator: action.prevalidator.clone(),
                    operation,
                    stateful: false,
                };
            } else {
                state.protocol.validation_state = ValidationState::Ready {
                    prevalidator: action.prevalidator.clone(),
                    stateful: false,
                };
            }
        }
        Action::ProtocolValidateOperationStart(action) => {
            let operation = action.operation.clone();
            match &state.protocol.validation_state {
                ValidationState::Ready {
                    stateful,
                    prevalidator,
                } => {
                    state.protocol.validation_state = ValidationState::Validating {
                        prevalidator: prevalidator.clone(),
                        operation,
                        stateful: *stateful,
                    };
                }
                _ => enqueue(state, QueueSide::Back, action.operation.clone()),
            }
        }
        Action::ProtocolValidateOperationDone(action) => {
            let stateful = match &state.protocol.validation_state {
                ValidationState::Validating { stateful, .. } => *stateful,
                _ => return, // the action is not allowed in such state
            };
            if let Some(operation) = dequeue(state) {
                state.protocol.validation_state = ValidationState::Validating {
                    prevalidator: action.response.prevalidator.clone(),
                    operation,
                    stateful,
                };
            } else {
                state.protocol.validation_state = ValidationState::Ready {
                    prevalidator: action.response.prevalidator.clone(),
                    stateful,
                };
            }
        }
        _ => {}
    }
}
