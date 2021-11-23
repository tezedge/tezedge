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
    PrecheckerOperationDecodedAction, PrecheckerOperationState, PrecheckerPrecheckOperationAction,
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
            prechecker_state
                .applied_blocks
                .entry(block_hash.clone())
                .or_insert(AppliedBlockCache {
                    chain_id: chain_id.clone(),
                    block_header: block_header.clone(),
                });
        }

        Action::PrecheckerPrecheckOperation(PrecheckerPrecheckOperationAction {
            key,
            block_hash,
            operation_binary_encoding,
        }) => {
            prechecker_state.operations.entry(key.clone()).or_insert(
                PrecheckerOperationState::Init {
                    block_hash: block_hash.clone(),
                    operation_binary_encoding: operation_binary_encoding.clone(),
                },
            );
        }

        Action::PrecheckerDecodeOperation(PrecheckerDecodeOperationAction { key }) => {
            prechecker_state
                .operations
                .entry(key.clone())
                .and_modify(|operation| {
                    if let PrecheckerOperationState::Init {
                        block_hash,
                        operation_binary_encoding,
                    } = operation
                    {
                        *operation = PrecheckerOperationState::PendingContentDecoding {
                            block_hash: block_hash.clone(),
                            operation_binary_encoding: operation_binary_encoding.clone(),
                        };
                    }
                });
        }
        Action::PrecheckerOperationDecoded(PrecheckerOperationDecodedAction { key, contents }) => {
            prechecker_state
                .operations
                .entry(key.clone())
                .and_modify(|operation| {
                    if let PrecheckerOperationState::PendingContentDecoding {
                        block_hash,
                        operation_binary_encoding,
                    } = operation
                    {
                        *operation = PrecheckerOperationState::DecodedContentReady {
                            block_hash: block_hash.clone(),
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
                .and_modify(|operation| {
                    if let PrecheckerOperationState::DecodedContentReady {
                        block_hash,
                        operation_binary_encoding,
                        operation_decoded_contents,
                    } = operation
                    {
                        *operation = PrecheckerOperationState::PendingBlockApplication {
                            block_hash: block_hash.clone(),
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
                .and_modify(|operation| {
                    if let PrecheckerOperationState::PendingBlockApplication {
                        block_hash,
                        operation_binary_encoding,
                        operation_decoded_contents,
                    } = operation
                    {
                        *operation = PrecheckerOperationState::BlockApplied {
                            block_hash: block_hash.clone(),
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
                .and_modify(|operation| {
                    if let PrecheckerOperationState::BlockApplied {
                        block_hash,
                        operation_binary_encoding,
                        operation_decoded_contents,
                    } = operation
                    {
                        *operation = PrecheckerOperationState::PendingEndorsingRights {
                            block_hash: block_hash.clone(),
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
                .and_modify(|operation| {
                    if let PrecheckerOperationState::PendingEndorsingRights {
                        block_hash,
                        operation_binary_encoding,
                        operation_decoded_contents,
                    } = operation
                    {
                        *operation = PrecheckerOperationState::EndorsingRightsReady {
                            block_hash: block_hash.clone(),
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
                .and_modify(|operation| {
                    if let PrecheckerOperationState::EndorsingRightsReady {
                        block_hash,
                        operation_binary_encoding,
                        operation_decoded_contents,
                        endorsing_rights,
                    } = operation
                    {
                        *operation = PrecheckerOperationState::PendingOperationPrechecking {
                            block_hash: block_hash.clone(),
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
                .and_modify(|operation| {
                    if let PrecheckerOperationState::PendingOperationPrechecking { .. } = operation
                    {
                        debug!(log, ">>> Prechecking successfull"; "operation" => key.to_string());
                        *operation = PrecheckerOperationState::Ready;
                    }
                });
        }
        Action::PrecheckerNotEndorsement(PrecheckerNotEndorsementAction { key }) => {
            let log = &state.log;
            prechecker_state
                .operations
                .entry(key.clone())
                .and_modify(|operation| {
                    if let PrecheckerOperationState::PendingContentDecoding { .. } = operation {
                        error!(log, ">>> Prechecking cannot be performed"; "operation" => key.to_string());
                        *operation = PrecheckerOperationState::NotEndorsement;
                    }
                });
        }
        Action::PrecheckerError(PrecheckerErrorAction { key, error }) => {
            let log = &state.log;
            prechecker_state
                .operations
                .entry(key.clone())
                .and_modify(|operation| {
                    if !matches!(operation, PrecheckerOperationState::Error { .. }) {
                        error!(log, ">>> Prechecking error"; "operation" => key.to_string(), "error" => error.to_string());
                        *operation = PrecheckerOperationState::Error {
                            error: error.clone(),
                        }
                    }
                });
        }

        _ => (),
    }
}
