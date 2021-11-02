// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use slog::{trace, warn, FnValue};

use crate::{Action, ActionWithMeta, State};

use super::{prechecker_actions::*, PrecheckerOperation, PrecheckerOperationState};

pub fn prechecker_reducer(state: &mut State, action: &ActionWithMeta) {
    let prechecker_state = &mut state.prechecker;
    match &action.action {
        Action::PrecheckerPrecheckOperationInit(PrecheckerPrecheckOperationInitAction {
            key,
            operation,
            operation_binary_encoding,
        }) => {
            prechecker_state
                .operations
                .entry(key.clone())
                .or_insert(PrecheckerOperation {
                    start: action.id,
                    operation: operation.clone(),
                    operation_binary_encoding: operation_binary_encoding.clone(),
                    state: PrecheckerOperationState::Init,
                });
        }

        Action::PrecheckerDecodeOperation(PrecheckerDecodeOperationAction { key }) => {
            prechecker_state
                .operations
                .entry(key.clone())
                .and_modify(|state| {
                    if let PrecheckerOperationState::Init = state.state {
                        state.state = PrecheckerOperationState::PendingContentDecoding;
                    }
                });
        }
        Action::PrecheckerOperationDecoded(PrecheckerOperationDecodedAction { key, contents }) => {
            prechecker_state
                .operations
                .entry(key.clone())
                .and_modify(|state| {
                    if let PrecheckerOperationState::PendingContentDecoding = state.state {
                        state.state = PrecheckerOperationState::DecodedContentReady {
                            operation_decoded_contents: contents.clone(),
                        };
                    }
                });
        }
        Action::PrecheckerGetEndorsingRights(PrecheckerGetEndorsingRightsAction { key }) => {
            prechecker_state
                .operations
                .entry(key.clone())
                .and_modify(|state| {
                    if let PrecheckerOperationState::DecodedContentReady {
                        operation_decoded_contents,
                    } = &state.state
                    {
                        state.state = PrecheckerOperationState::PendingEndorsingRights {
                            operation_decoded_contents: operation_decoded_contents.clone(),
                        }
                    }
                });
        }
        Action::PrecheckerEndorsingRightsReady(PrecheckerEndorsingRightsReadyAction {
            key,
            endorsing_rights,
        }) => {
            prechecker_state
                .operations
                .entry(key.clone())
                .and_modify(|state| {
                    if let PrecheckerOperationState::PendingEndorsingRights {
                        operation_decoded_contents,
                    } = &state.state
                    {
                        state.state = PrecheckerOperationState::EndorsingRightsReady {
                            operation_decoded_contents: operation_decoded_contents.clone(),
                            endorsing_rights: endorsing_rights.clone(),
                        }
                    }
                });
        }
        Action::PrecheckerValidateEndorsement(PrecheckerValidateEndorsementAction { key }) => {
            prechecker_state
                .operations
                .entry(key.clone())
                .and_modify(|state| {
                    if let PrecheckerOperationState::EndorsingRightsReady {
                        operation_decoded_contents,
                        endorsing_rights,
                    } = &state.state
                    {
                        state.state = PrecheckerOperationState::PendingOperationPrechecking {
                            operation_decoded_contents: operation_decoded_contents.clone(),
                            endorsing_rights: endorsing_rights.clone(),
                        }
                    }
                });
        }
        Action::PrecheckerEndorsementValidationApplied(
            PrecheckerEndorsementValidationAppliedAction { key, protocol_data },
        ) => {
            let log = &state.log;
            prechecker_state
                .operations
                .entry(key.clone())
                .and_modify(|state| {
                    if let PrecheckerOperationState::PendingOperationPrechecking { .. } =
                        &state.state
                    {
                        trace!(log, "Prechecking  successfull";
                               "operation" => FnValue(|_| key.operation.to_base58_check()),
                               "duration" => FnValue(|_| format!("{:?}", action.id.duration_since(state.start)))
                        );
                        state.state = PrecheckerOperationState::Applied {
                            protocol_data: protocol_data.clone(),
                        };
                    }
                });
        }
        Action::PrecheckerEndorsementValidationRefused(
            PrecheckerEndorsementValidationRefusedAction {
                key,
                protocol_data,
                error,
            },
        ) => {
            let log = &state.log;
            prechecker_state
                .operations
                .entry(key.clone())
                .and_modify(|state| {
                    if let PrecheckerOperationState::PendingOperationPrechecking { .. } =
                        &state.state
                    {
                        trace!(log, "Prechecking refused";
                               "operation" => FnValue(|_| key.operation.to_string()),
                               "duration" => FnValue(|_| format!("{:?}", action.id.duration_since(state.start)))
                        );
                        state.state = PrecheckerOperationState::Refused {
                            protocol_data: protocol_data.clone(),
                            error: error.clone(),
                        };
                    }
                });
        }
        Action::PrecheckerProtocolNeeded(PrecheckerProtocolNeededAction { key, .. }) => {
            let log = &state.log;
            prechecker_state
                .operations
                .entry(key.clone())
                .and_modify(|state| {
                    match &state.state {
                        PrecheckerOperationState::DecodedContentReady { .. } => {
                            trace!(log, "Prechecking cannot be performed (not an endorsement)";
                                   "operation" => FnValue(|_| key.operation.to_string()),
                                   "duration" => FnValue(|_| format!("{:?}", action.id.duration_since(state.start)))
                            );
                            state.state = PrecheckerOperationState::ProtocolNeeded;
                        }
                        PrecheckerOperationState::PendingOperationPrechecking { .. } => {
                            trace!(log, "Prechecking cannot be performed (unsupported ECDSA)";
                                   "operation" => FnValue(|_| key.operation.to_string()),
                                   "duration" => FnValue(|_| format!("{:?}", action.id.duration_since(state.start)))
                            );
                            state.state = PrecheckerOperationState::ProtocolNeeded;
                        }
                        _ => (),
                    }
                });
        }
        Action::PrecheckerError(PrecheckerErrorAction { key, error }) => {
            let log = &state.log;
            prechecker_state
                .operations
                .entry(key.clone())
                .and_modify(|state| {
                    if !matches!(&state.state, PrecheckerOperationState::Error { .. }) {
                        warn!(log, "Prechecking error";
                              "operation" => FnValue(|_| key.to_string()),
                              "error" => FnValue(|_| error.to_string())
                        );
                        state.state = PrecheckerOperationState::Error {
                            error: error.clone(),
                        }
                    }
                });
        }

        _ => (),
    }
}
