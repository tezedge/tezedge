// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::ActionWithMeta;
use slog::{debug, error};

use crate::{Action, State};

use super::{
    AppliedBlockCache, PrecheckerBlockAppliedAction, PrecheckerCacheAppliedBlockAction,
    PrecheckerDecodeOperationAction, PrecheckerEndorsementValidationReadyAction,
    PrecheckerEndorsingRightsReadyAction, PrecheckerErrorAction,
    PrecheckerGetEndorsingRightsAction, PrecheckerNotEndorsementAction,
    PrecheckerOperationDecodedAction, PrecheckerOperationState,
    PrecheckerPrecheckOperationInitAction, PrecheckerValidateEndorsementAction,
    PrecheckerWaitForBlockApplicationAction,
};

pub fn prechecker_reducer(state: &mut State, action: &ActionWithMeta) {
    let prechecker_state = &mut state.prechecker;
    match &action.action {
        Action::PrecheckerCacheAppliedBlock(PrecheckerCacheAppliedBlockAction {
            block_hash,
            chain_id,
            block_header,
        }) => {
            prechecker_state
                .applied_blocks
                .entry(block_hash.clone())
                .or_insert(AppliedBlockCache {
                    chain_id: chain_id.clone(),
                    block_header: block_header.clone(),
                });
        }

        Action::PrecheckerPrecheckOperationInit(PrecheckerPrecheckOperationInitAction {
            key,
            operation,
            operation_binary_encoding,
        }) => {
            prechecker_state.operations.entry(key.clone()).or_insert(
                PrecheckerOperationState::Init {
                    operation: operation.clone(),
                    operation_binary_encoding: operation_binary_encoding.clone(),
                },
            );
        }

        Action::PrecheckerDecodeOperation(PrecheckerDecodeOperationAction { key }) => {
            prechecker_state
                .operations
                .entry(key.clone())
                .and_modify(|state| {
                    if let PrecheckerOperationState::Init {
                        operation,
                        operation_binary_encoding,
                    } = state
                    {
                        *state = PrecheckerOperationState::PendingContentDecoding {
                            operation: operation.clone(),
                            operation_binary_encoding: operation_binary_encoding.clone(),
                        };
                    }
                });
        }
        Action::PrecheckerOperationDecoded(PrecheckerOperationDecodedAction { key, contents }) => {
            prechecker_state
                .operations
                .entry(key.clone())
                .and_modify(|state| {
                    if let PrecheckerOperationState::PendingContentDecoding { operation, operation_binary_encoding } = state {
                        *state = PrecheckerOperationState::DecodedContentReady {
                            operation: operation.clone(),
                            operation_binary_encoding: operation_binary_encoding.clone(),
                            operation_decoded_contents: contents.clone(),
                        };
                    }
                });
        }
        Action::PrecheckerWaitForBlockApplication(PrecheckerWaitForBlockApplicationAction {
            key,
        }) => {
            prechecker_state
                .operations
                .entry(key.clone())
                .and_modify(|state| {
                    if let PrecheckerOperationState::DecodedContentReady {
                        operation,
                        operation_binary_encoding,
                        operation_decoded_contents,
                    } = state
                    {
                        *state = PrecheckerOperationState::PendingBlockApplication {
                            operation: operation.clone(),
                            operation_binary_encoding: operation_binary_encoding.clone(),
                            operation_decoded_contents: operation_decoded_contents.clone(),
                        };
                    }
                });
        }
        Action::PrecheckerBlockApplied(PrecheckerBlockAppliedAction { key }) => {
            prechecker_state
                .operations
                .entry(key.clone())
                .and_modify(|state| {
                    if let PrecheckerOperationState::PendingBlockApplication {
                        operation,
                        operation_binary_encoding,
                        operation_decoded_contents,
                    } = state
                    {
                        *state = PrecheckerOperationState::BlockApplied {
                            operation: operation.clone(),
                            operation_binary_encoding: operation_binary_encoding.clone(),
                            operation_decoded_contents: operation_decoded_contents.clone(),
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
                        operation,
                        operation_binary_encoding,
                        operation_decoded_contents,
                    } = state
                    {
                        *state = PrecheckerOperationState::PendingEndorsingRights {
                            operation: operation.clone(),
                            operation_binary_encoding: operation_binary_encoding.clone(),
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
                        operation,
                        operation_binary_encoding,
                        operation_decoded_contents,
                    } = state
                    {
                        *state = PrecheckerOperationState::EndorsingRightsReady {
                            operation: operation.clone(),
                            operation_binary_encoding: operation_binary_encoding.clone(),
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
                        operation,
                        operation_binary_encoding,
                        operation_decoded_contents,
                        endorsing_rights,
                    } = state
                    {
                        *state = PrecheckerOperationState::PendingOperationPrechecking {
                            operation: operation.clone(),
                            operation_binary_encoding: operation_binary_encoding.clone(),
                            operation_decoded_contents: operation_decoded_contents.clone(),
                            endorsing_rights: endorsing_rights.clone(),
                        }
                    }
                });
        }
        Action::PrecheckerEndorsementValidationReady(
            PrecheckerEndorsementValidationReadyAction { key },
        ) => {
            let log = &state.log;
            prechecker_state
                .operations
                .entry(key.clone())
                .and_modify(|state| {
                    if let PrecheckerOperationState::PendingOperationPrechecking { .. } = state {
                        debug!(log, ">>> Prechecking successfull"; "operation" => key.to_string());
                        *state = PrecheckerOperationState::Ready;
                    }
                });
        }
        Action::PrecheckerNotEndorsement(PrecheckerNotEndorsementAction { key }) => {
            let log = &state.log;
            prechecker_state
                .operations
                .entry(key.clone())
                .and_modify(|state| {
                    if let PrecheckerOperationState::PendingContentDecoding { .. } = state {
                        error!(log, ">>> Prechecking cannot be performed"; "operation" => key.to_string());
                        *state = PrecheckerOperationState::NotEndorsement;
                    }
                });
        }
        Action::PrecheckerError(PrecheckerErrorAction { key, error }) => {
            let log = &state.log;
            prechecker_state
                .operations
                .entry(key.clone())
                .and_modify(|state| {
                    if !matches!(state, PrecheckerOperationState::Error { .. }) {
                        error!(log, ">>> Prechecking error"; "operation" => key.to_string(), "error" => error.to_string());
                        *state = PrecheckerOperationState::Error {
                            operation: state.operation().cloned(),
                            error: error.clone(),
                        }
                    }
                });
        }

        _ => (),
    }
}
