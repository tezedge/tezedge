// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use redux_rs::ActionWithId;

use crate::{Action, State};

use super::{PrecheckerDecodeOperationAction, PrecheckerEndorsementValidationReadyAction, PrecheckerEndorsingRightsReadyAction, PrecheckerErrorAction, PrecheckerGetEndorsingRightsAction, PrecheckerOperationDecodedAction, PrecheckerOperationState, PrecheckerPrecheckOperationAction, PrecheckerValidateEndorsementAction};

pub fn prechecker_reducer(state: &mut State, action: &ActionWithId<Action>) {
    let prechecker_state = &mut state.prechecker;
    match &action.action {
        Action::PrecheckerPrecheckOperation(PrecheckerPrecheckOperationAction { key }) => {
            prechecker_state
                .operations
                .entry(key.clone())
                .or_insert(PrecheckerOperationState::Init);
        }

        Action::PrecheckerDecodeOperation(PrecheckerDecodeOperationAction { key }) => {
            prechecker_state
                .operations
                .entry(key.clone())
                .and_modify(|operation| {
                    if let PrecheckerOperationState::Init = operation {
                        *operation = PrecheckerOperationState::PendingContentDecoding;
                    }
                });
        }
        Action::PrecheckerOperationDecoded(PrecheckerOperationDecodedAction { key, contents }) => {
            prechecker_state
                .operations
                .entry(key.clone())
                .and_modify(|operation| {
                    if let PrecheckerOperationState::PendingContentDecoding = operation {
                        *operation = PrecheckerOperationState::DecodedContentReady {
                            operation_decoded_contents: contents.clone(),
                        }
                    }
                });
        }
        Action::PrecheckerGetEndorsingRights(PrecheckerGetEndorsingRightsAction { key }) => {
            prechecker_state
                .operations
                .entry(key.clone())
                .and_modify(|operation| {
                    if let PrecheckerOperationState::DecodedContentReady {
                        operation_decoded_contents,
                    } = operation
                    {
                        *operation = PrecheckerOperationState::PendingEndorsingRights {
                            operation_decoded_contents: operation_decoded_contents.clone(),
                        }
                    }
                });
        }
        Action::PrecheckerEndorsingRightsReady(PrecheckerEndorsingRightsReadyAction { key }) => {
            prechecker_state
                .operations
                .entry(key.clone())
                .and_modify(|operation| {
                    if let PrecheckerOperationState::PendingEndorsingRights {
                        operation_decoded_contents,
                    } = operation
                    {
                        *operation = PrecheckerOperationState::EndorsingRightsReady {
                            operation_decoded_contents: operation_decoded_contents.clone(),
                        }
                    }
                });
        }
        Action::PrecheckerValidateEndorsement(PrecheckerValidateEndorsementAction { key }) => {
            prechecker_state
                .operations
                .entry(key.clone())
                .and_modify(|operation| {
                    if let PrecheckerOperationState::EndorsingRightsReady {
                        operation_decoded_contents,
                    } = operation
                    {
                        *operation = PrecheckerOperationState::PendingOperationPrechecking {
                            operation_decoded_contents: operation_decoded_contents.clone(),
                        }
                    }
                });
        }
        Action::PrecheckerEndorsementValidationReady(PrecheckerEndorsementValidationReadyAction { key, result }) => {
            prechecker_state
                .operations
                .entry(key.clone())
                .and_modify(|operation| {
                    if let PrecheckerOperationState::PendingOperationPrechecking {
                        operation_decoded_contents,
                    } = operation
                    {
                        *operation = PrecheckerOperationState::Ready {
                            verdict: result.clone(),
                        }
                    }
                });
        }
        Action::PrecheckerError(PrecheckerErrorAction { key, error }) => {
            prechecker_state
                .operations
                .entry(key.clone())
                .and_modify(|operation| {
                    *operation = PrecheckerOperationState::Error {
                        error: error.clone(),
                    }
                });
        }




        _ => (),
    }
}
