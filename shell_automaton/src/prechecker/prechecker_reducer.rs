// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::ActionWithMeta;
use slog::{debug, error};

use crate::{Action, State};

use super::{
    CurrentBlock, PrecheckerBlockAppliedAction, PrecheckerCacheAppliedBlockAction,
    PrecheckerDecodeOperationAction, PrecheckerEndorsementValidationAppliedAction,
    PrecheckerEndorsementValidationRefusedAction, PrecheckerEndorsingRightsReadyAction,
    PrecheckerErrorAction, PrecheckerGetEndorsingRightsAction, PrecheckerOperation,
    PrecheckerOperationDecodedAction, PrecheckerOperationState,
    PrecheckerPrecheckOperationInitAction, PrecheckerProtocolNeededAction,
    PrecheckerValidateEndorsementAction, PrecheckerWaitForBlockApplicationAction,
};

pub fn prechecker_reducer(state: &mut State, action: &ActionWithMeta) {
    let prechecker_state = &mut state.prechecker;
    match &action.action {
        Action::PrecheckerCacheAppliedBlock(PrecheckerCacheAppliedBlockAction {
            block_hash,
            chain_id,
            block_header,
        }) => {
            prechecker_state.current_block = Some(CurrentBlock {
                chain_id: chain_id.clone(),
                block_hash: block_hash.clone(),
                block_header: block_header.clone(),
            })
        }

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
        Action::PrecheckerWaitForBlockApplication(PrecheckerWaitForBlockApplicationAction {
            key,
            level,
        }) => {
            prechecker_state
                .operations
                .entry(key.clone())
                .and_modify(|state| {
                    if let PrecheckerOperationState::DecodedContentReady {
                        operation_decoded_contents,
                    } = &state.state
                    {
                        state.state = PrecheckerOperationState::PendingBlockApplication {
                            operation_decoded_contents: operation_decoded_contents.clone(),
                            level: *level,
                        };
                    }
                });
        }
        Action::PrecheckerBlockApplied(PrecheckerBlockAppliedAction { key, .. }) => {
            prechecker_state
                .operations
                .entry(key.clone())
                .and_modify(|state| {
                    if let PrecheckerOperationState::PendingBlockApplication {
                        operation_decoded_contents,
                        level,
                    } = &state.state
                    {
                        state.state = PrecheckerOperationState::BlockApplied {
                            operation_decoded_contents: operation_decoded_contents.clone(),
                            level: *level,
                        }
                    }
                });
        }
        Action::PrecheckerGetEndorsingRights(PrecheckerGetEndorsingRightsAction { key }) => {
            prechecker_state
                .operations
                .entry(key.clone())
                .and_modify(|state| {
                    if let PrecheckerOperationState::BlockApplied {
                        operation_decoded_contents,
                        ..
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
                        debug!(log, ">>> Prechecking  successfull"; "operation" => key.operation.to_string(), "duration" => format!("{:?}", action.id.duration_since(state.start)));
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
                        debug!(log, ">>> Prechecking refused"; "operation" => key.operation.to_string(), "duration" => format!("{:?}", action.id.duration_since(state.start)));
                        state.state = PrecheckerOperationState::Refused {
                            protocol_data: protocol_data.clone(),
                            error: error.clone(),
                        };
                    }
                });
        }
        Action::PrecheckerProtocolNeeded(PrecheckerProtocolNeededAction { key }) => {
            let log = &state.log;
            prechecker_state
                .operations
                .entry(key.clone())
                .and_modify(|state| {
                    if let PrecheckerOperationState::PendingContentDecoding { .. } = &state.state {
                        debug!(log, ">>> Prechecking cannot be performed"; "operation" => key.operation.to_string(), "duration" => format!("{:?}", action.id.duration_since(state.start)));
                        state.state = PrecheckerOperationState::ProtocolNeeded;
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
                        error!(log, ">>> Prechecking error"; "operation" => key.to_string(), "error" => error.to_string());
                        state.state = PrecheckerOperationState::Error {
                            error: error.clone(),
                        }
                    }
                });
        }

        _ => (),
    }
}
