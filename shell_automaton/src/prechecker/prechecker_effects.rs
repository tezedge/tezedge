// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::convert::TryInto;

use crypto::{blake2b, hash::BlockHash};
use slog::{debug, error};
use tezos_messages::p2p::{
    binary_message::{BinaryWrite, MessageHash, MessageHashError},
    encoding::block_header::BlockHeader,
};

use crate::{
    mempool::BlockAppliedAction,
    prechecker::{Applied, CurrentBlock, PrecheckerEndorsementValidationRefusedAction, Refused},
    rights::{
        EndorsingRightsKey, RightsEndorsingRightsErrorAction, RightsEndorsingRightsReadyAction,
        RightsGetEndorsingRightsAction,
    },
    Action, ActionWithMeta, Service, Store,
};

use super::{
    EndorsementValidationError, Key, OperationDecodedContents, PrecheckerBlockAppliedAction,
    PrecheckerCacheAppliedBlockAction, PrecheckerDecodeOperationAction,
    PrecheckerEndorsementValidationAppliedAction, PrecheckerEndorsingRightsReadyAction,
    PrecheckerError, PrecheckerErrorAction, PrecheckerGetEndorsingRightsAction,
    PrecheckerOperation, PrecheckerOperationDecodedAction, PrecheckerOperationState,
    PrecheckerPrecheckOperationInitAction, PrecheckerPrecheckOperationRequestAction,
    PrecheckerPrecheckOperationResponseAction, PrecheckerProtocolNeededAction,
    PrecheckerValidateEndorsementAction, PrecheckerWaitForBlockApplicationAction,
};

pub fn prechecker_effects<S>(store: &mut Store<S>, action: &ActionWithMeta)
where
    S: Service,
{
    let prechecker_state = &store.state.get().prechecker;
    let prechecker_state_operations = &prechecker_state.operations;
    let log = &store.state.get().log;
    match &action.action {
        Action::BlockApplied(BlockAppliedAction {
            block,
            chain_id,
            is_bootstrapped,
        }) => {
            let block_hash = match get_block_hash(&block) {
                Ok(block_hash) => block_hash,
                Err(err) => {
                    error!(log, "Cannot cache applied block at level {}", block.level(); "error" => err.to_string());
                    return;
                }
            };

            debug!(log, "New block applied"; "block_hash" => block_hash.to_string());

            store.dispatch(PrecheckerCacheAppliedBlockAction {
                block_hash: block_hash.clone(),
                chain_id: chain_id.clone(),
                block_header: block.clone(),
            });

            if !is_bootstrapped {
                return;
            }

            for key in &store
                .state
                .get()
                .prechecker
                .operations
                .iter()
                .filter_map(|(key, state)| {
                    if let PrecheckerOperationState::PendingBlockApplication { level, .. } =
                        state.state
                    {
                        if level == block.level() {
                            Some(key)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .cloned()
                .collect::<Vec<_>>()
            {
                store.dispatch(PrecheckerBlockAppliedAction { key: key.clone() });
            }
        }

        // debug only
        Action::PrecheckerCacheAppliedBlock(PrecheckerCacheAppliedBlockAction {
            block_hash,
            ..
        }) => {
            for (key, state) in prechecker_state
                .non_terminals()
                .filter(|(_, state)| state.operation.branch() != block_hash)
            {
                debug!(log, "Prevalidation operation still unprocessed";
                           "operation" => key.operation.to_string(), "state" => state.state.as_ref(), "block_hash" => state.block_hash().to_string());
            }
        }

        Action::PrecheckerPrecheckOperationRequest(PrecheckerPrecheckOperationRequestAction {
            operation,
        }) => {
            let binary_encoding = match operation.as_bytes() {
                Ok(bytes) => bytes,
                Err(err) => {
                    store.dispatch(PrecheckerPrecheckOperationResponseAction::error(err));
                    return;
                }
            };

            let hash = match blake2b::digest_256(&binary_encoding) {
                Ok(hash) => hash,
                Err(err) => {
                    store.dispatch(PrecheckerPrecheckOperationResponseAction::error(err));
                    return;
                }
            };
            let key = match hash.try_into() {
                Ok(hash) => Key { operation: hash },
                Err(err) => {
                    store.dispatch(PrecheckerPrecheckOperationResponseAction::error(err));
                    return;
                }
            };
            store.dispatch(PrecheckerPrecheckOperationInitAction {
                key,
                operation: operation.clone(),
                operation_binary_encoding: binary_encoding,
            });
        }
        Action::PrecheckerPrecheckOperationInit(PrecheckerPrecheckOperationInitAction {
            key,
            ..
        }) => {
            match prechecker_state_operations.get(key).map(|op| &op.state) {
                Some(PrecheckerOperationState::Init { .. }) => {
                    store.dispatch(PrecheckerDecodeOperationAction { key: key.clone() });
                }
                Some(PrecheckerOperationState::Applied { protocol_data }) => {
                    let action = PrecheckerEndorsementValidationAppliedAction {
                        key: key.clone(),
                        protocol_data: protocol_data.clone(),
                    };
                    store.dispatch(action);
                }
                Some(PrecheckerOperationState::Error { error, .. }) => {
                    let error = error.clone();
                    store.dispatch(PrecheckerErrorAction::new(key.clone(), error));
                }
                _ => (),
            };
        }
        Action::PrecheckerDecodeOperation(PrecheckerDecodeOperationAction { key }) => {
            if let Some(PrecheckerOperation {
                operation_binary_encoding,
                state: PrecheckerOperationState::PendingContentDecoding,
                ..
            }) = prechecker_state_operations.get(key)
            {
                // TODO use proper protocol to parse operation
                match OperationDecodedContents::parse(&operation_binary_encoding) {
                    Ok(contents) if contents.is_endorsement() => {
                        store.dispatch(PrecheckerOperationDecodedAction {
                            key: key.clone(),
                            contents,
                        });
                    }
                    Ok(_) => {
                        store.dispatch(PrecheckerProtocolNeededAction { key: key.clone() });
                    }
                    Err(err) => {
                        store.dispatch(PrecheckerErrorAction::new(key.clone(), err));
                    }
                }
            }
        }
        Action::PrecheckerOperationDecoded(PrecheckerOperationDecodedAction { key, .. }) => {
            if let Some(PrecheckerOperationState::DecodedContentReady {
                operation_decoded_contents,
                ..
            }) = prechecker_state_operations.get(key).map(|op| &op.state)
            {
                if let Some(level) = operation_decoded_contents.endorsement_level() {
                    if let Some(current_level) = prechecker_state
                        .current_block
                        .as_ref()
                        .map(|cb| cb.block_header.level())
                    {
                        if level == current_level || level == current_level + 1 {
                            store.dispatch(PrecheckerWaitForBlockApplicationAction {
                                key: key.clone(),
                                level,
                            });
                        } else {
                            let protocol_data = operation_decoded_contents.as_json();
                            store.dispatch(PrecheckerEndorsementValidationRefusedAction {
                                key: key.clone(),
                                error: EndorsementValidationError::InvalidLevel,
                                protocol_data,
                            });
                        }
                    }
                }
            }
        }
        Action::PrecheckerWaitForBlockApplication(PrecheckerWaitForBlockApplicationAction {
            key,
            level,
        }) => {
            if let Some(PrecheckerOperationState::PendingBlockApplication { .. }) =
                prechecker_state_operations.get(key).map(|op| &op.state)
            {
                if prechecker_state
                    .current_block
                    .as_ref()
                    .map(|cb| cb.block_header.level() == *level)
                    .unwrap_or(false)
                {
                    store.dispatch(PrecheckerBlockAppliedAction { key: key.clone() });
                }
            }
        }
        Action::PrecheckerBlockApplied(PrecheckerBlockAppliedAction { key }) => {
            if let Some(PrecheckerOperationState::BlockApplied { .. }) =
                prechecker_state_operations.get(key).map(|op| &op.state)
            {
                store.dispatch(PrecheckerGetEndorsingRightsAction { key: key.clone() });
            }
        }

        Action::PrecheckerGetEndorsingRights(PrecheckerGetEndorsingRightsAction { key }) => {
            if let Some(PrecheckerOperationState::PendingEndorsingRights {
                operation_decoded_contents,
                ..
            }) = prechecker_state_operations.get(key).map(|op| &op.state)
            {
                if let Some(current_block_hash) = prechecker_state
                    .current_block
                    .as_ref()
                    .map(|cb| &cb.block_hash)
                {
                    if let Some(level) = operation_decoded_contents.endorsement_level() {
                        let current_block_hash = current_block_hash.clone();
                        store.dispatch(RightsGetEndorsingRightsAction {
                            key: EndorsingRightsKey {
                                current_block_hash,
                                level: Some(level),
                            },
                        });
                    }
                }
            }
        }
        Action::RightsEndorsingRightsReady(RightsEndorsingRightsReadyAction {
            key: EndorsingRightsKey {
                level: Some(level), ..
            },
            endorsing_rights,
        }) => {
            for key in prechecker_state_operations
                .iter()
                .filter_map(|(key, state)| {
                    if let PrecheckerOperationState::PendingEndorsingRights {
                        operation_decoded_contents,
                        ..
                    } = &state.state
                    {
                        if operation_decoded_contents
                            .endorsement_level()
                            .map(|l| l == *level)
                            .unwrap_or(false)
                        {
                            Some(key)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .cloned()
                .collect::<Vec<_>>()
            {
                store.dispatch(PrecheckerEndorsingRightsReadyAction {
                    key,
                    endorsing_rights: endorsing_rights.clone(),
                });
            }
        }
        Action::RightsEndorsingRightsError(RightsEndorsingRightsErrorAction {
            key:
                EndorsingRightsKey {
                    current_block_hash,
                    level: None,
                },
            error,
        }) => {
            for key in prechecker_state_operations
                .iter()
                .filter_map(|(key, state)| {
                    if let PrecheckerOperationState::PendingEndorsingRights { .. } = state.state {
                        if state.operation.branch() == current_block_hash {
                            Some(key)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .cloned()
                .collect::<Vec<_>>()
            {
                store.dispatch(PrecheckerErrorAction::new(key, error.clone()));
            }
        }
        Action::PrecheckerEndorsingRightsReady(PrecheckerEndorsingRightsReadyAction {
            key,
            ..
        }) => {
            if let Some(PrecheckerOperationState::EndorsingRightsReady { .. }) =
                prechecker_state_operations.get(key).map(|op| &op.state)
            {
                store.dispatch(PrecheckerValidateEndorsementAction { key: key.clone() });
            }
        }
        Action::PrecheckerValidateEndorsement(PrecheckerValidateEndorsementAction { key }) => {
            if let Some(PrecheckerOperation {
                operation_binary_encoding,
                state:
                    PrecheckerOperationState::PendingOperationPrechecking {
                        operation_decoded_contents,
                        endorsing_rights,
                    },
                ..
            }) = prechecker_state_operations.get(key)
            {
                let (chain_id, block_hash) =
                    match store.state.get().prechecker.current_block.as_ref() {
                        Some(CurrentBlock {
                            chain_id,
                            block_hash,
                            ..
                        }) => (chain_id, block_hash),
                        _ => return,
                    };
                use super::EndorsementValidator;
                let validation_result = match operation_decoded_contents {
                    OperationDecodedContents::Proto010(operation) => operation
                        .validate_endorsement(
                            operation_binary_encoding,
                            chain_id,
                            block_hash,
                            endorsing_rights,
                            log,
                        ),
                };

                match validation_result {
                    Ok(Applied { protocol_data }) => {
                        store.dispatch(PrecheckerEndorsementValidationAppliedAction {
                            key: key.clone(),
                            protocol_data,
                        })
                    }
                    Err(Refused {
                        protocol_data,
                        error,
                    }) => store.dispatch(PrecheckerEndorsementValidationRefusedAction {
                        key: key.clone(),
                        protocol_data,
                        error,
                    }),
                };
            }
        }
        Action::PrecheckerEndorsementValidationApplied(
            PrecheckerEndorsementValidationAppliedAction { key, protocol_data },
        ) => {
            if let Some(PrecheckerOperationState::Applied { .. }) =
                prechecker_state_operations.get(key).map(|op| &op.state)
            {
                store.dispatch(PrecheckerPrecheckOperationResponseAction::valid(
                    &key.operation,
                    protocol_data.clone(),
                ));
            }
        }
        Action::PrecheckerEndorsementValidationRefused(
            PrecheckerEndorsementValidationRefusedAction { key, .. },
        ) => {
            if let Some(PrecheckerOperation {
                state:
                    PrecheckerOperationState::Refused {
                        protocol_data,
                        error,
                    },
                operation,
                ..
            }) = prechecker_state_operations.get(key)
            {
                match error {
                    EndorsementValidationError::UnsupportedPublicKey => {
                        let action =
                            PrecheckerPrecheckOperationResponseAction::prevalidate(operation);
                        store.dispatch(action);
                    }
                    _ => {
                        let action = PrecheckerPrecheckOperationResponseAction::reject(
                            &key.operation,
                            protocol_data.clone(),
                            serde_json::to_string(error).unwrap_or("<unserialized>".to_string()),
                        );
                        store.dispatch(action);
                    }
                }
            }
        }
        Action::PrecheckerError(PrecheckerErrorAction { key, error }) => {
            if let Some(PrecheckerOperationState::Error { .. }) =
                prechecker_state_operations.get(key).map(|op| &op.state)
            {
                match error {
                    PrecheckerError::EndorsingRights(err) => {
                        error!(log, "Getting endorsing rights failed"; "operation" => key.to_string(), "error" => err.to_string());
                        store.dispatch(PrecheckerPrecheckOperationResponseAction::error(
                            err.clone(),
                        ));
                    }
                    PrecheckerError::OperationContentsDecode(err) => {
                        store.dispatch(PrecheckerPrecheckOperationResponseAction::error(
                            err.clone(),
                        ));
                    }
                }
            }
        }
        _ => (),
    }
}

fn get_block_hash(block_header: &BlockHeader) -> Result<BlockHash, MessageHashError> {
    let block_hash: BlockHash = if let Some(hash) = block_header.hash().as_ref() {
        hash.as_slice().try_into()?
    } else {
        block_header.message_hash()?.try_into()?
    };

    Ok(block_hash)
}
