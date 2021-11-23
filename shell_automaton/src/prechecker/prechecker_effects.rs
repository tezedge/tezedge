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
    mempool::{BlockAppliedAction, MempoolOperationRecvDoneAction},
    prechecker::AppliedBlockCache,
    rights::{
        EndorsingRightsKey, RightsEndorsingRightsErrorAction, RightsEndorsingRightsReadyAction,
        RightsGetEndorsingRightsAction,
    },
    Action, ActionWithMeta, Service, Store,
};

use super::{
    Key, OperationDecodedContents, PrecheckerBlockAppliedAction, PrecheckerCacheAppliedBlockAction,
    PrecheckerDecodeOperationAction, PrecheckerEndorsementValidationReadyAction,
    PrecheckerEndorsingRightsReadyAction, PrecheckerErrorAction,
    PrecheckerGetEndorsingRightsAction, PrecheckerInitErrorAction, PrecheckerNotEndorsementAction,
    PrecheckerOperationDecodedAction, PrecheckerOperationState, PrecheckerPrecheckOperationAction,
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
            if !is_bootstrapped {
                return;
            }
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

            for key in &store
                .state
                .get()
                .prechecker
                .operations
                .iter()
                .filter_map(|(key, state)| {
                    if let PrecheckerOperationState::PendingBlockApplication {
                        block_hash: operation_block_hash,
                        ..
                    } = state
                    {
                        if &block_hash == operation_block_hash {
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
            for (key, state) in prechecker_state.non_terminals() {
                state.block_hash().map(|bh| if bh != block_hash {
                    debug!(log, "Prevalidation operation still unprocessed";
                           "operation" => key.operation.to_string(), "state" => state.as_ref(), "block_hash" => bh.to_string());
                });
            }
        }

        Action::MempoolOperationRecvDone(MempoolOperationRecvDoneAction {
            address: _,
            operation,
        }) => {
            let binary_encoding = match operation.as_bytes() {
                Ok(bytes) => bytes,
                Err(err) => {
                    store.dispatch(PrecheckerInitErrorAction { error: err.into() });
                    return;
                }
            };

            let hash = match blake2b::digest_256(&binary_encoding) {
                Ok(hash) => hash,
                Err(err) => {
                    store.dispatch(PrecheckerInitErrorAction { error: err.into() });
                    return;
                }
            };
            let key = match hash.try_into() {
                Ok(hash) => Key { operation: hash },
                Err(err) => {
                    store.dispatch(PrecheckerInitErrorAction { error: err.into() });
                    return;
                }
            };
            store.dispatch(PrecheckerPrecheckOperationAction {
                key,
                block_hash: operation.branch().clone(),
                operation_binary_encoding: binary_encoding,
            });
        }
        Action::PrecheckerPrecheckOperation(PrecheckerPrecheckOperationAction { key, .. }) => {
            match prechecker_state_operations.get(key) {
                Some(PrecheckerOperationState::Init { .. }) => {
                    store.dispatch(PrecheckerDecodeOperationAction { key: key.clone() });
                }
                Some(PrecheckerOperationState::Ready {}) => {
                    store.dispatch(PrecheckerEndorsementValidationReadyAction { key: key.clone() });
                }
                Some(PrecheckerOperationState::Error { error }) => {
                    let error = error.clone();
                    store.dispatch(PrecheckerErrorAction {
                        key: key.clone(),
                        error,
                    });
                }
                _ => (),
            };
        }
        Action::PrecheckerDecodeOperation(PrecheckerDecodeOperationAction { key }) => {
            if let Some(PrecheckerOperationState::PendingContentDecoding {
                operation_binary_encoding,
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
                        store.dispatch(PrecheckerNotEndorsementAction { key: key.clone() });
                    }
                    Err(err) => {
                        store.dispatch(PrecheckerErrorAction {
                            key: key.clone(),
                            error: err.into(),
                        });
                    }
                }
            }
        }
        Action::PrecheckerOperationDecoded(PrecheckerOperationDecodedAction { key, .. }) => {
            if let Some(PrecheckerOperationState::DecodedContentReady { .. }) =
                prechecker_state_operations.get(key)
            {
                store.dispatch(PrecheckerWaitForBlockApplicationAction { key: key.clone() });
            }
        }
        Action::PrecheckerWaitForBlockApplication(PrecheckerWaitForBlockApplicationAction {
            key,
        }) => {
            if let Some(PrecheckerOperationState::PendingBlockApplication { block_hash, .. }) =
                prechecker_state_operations.get(key)
            {
                if prechecker_state.applied_blocks.contains_key(block_hash) {
                    store.dispatch(PrecheckerBlockAppliedAction { key: key.clone() });
                }
            }
        }
        Action::PrecheckerBlockApplied(PrecheckerBlockAppliedAction { key }) => {
            if let Some(PrecheckerOperationState::BlockApplied { .. }) =
                prechecker_state_operations.get(key)
            {
                store.dispatch(PrecheckerGetEndorsingRightsAction { key: key.clone() });
            }
        }

        Action::PrecheckerGetEndorsingRights(PrecheckerGetEndorsingRightsAction { key }) => {
            if let Some(PrecheckerOperationState::PendingEndorsingRights { block_hash, .. }) =
                prechecker_state_operations.get(key)
            {
                let current_block_hash = block_hash.clone();
                store.dispatch(RightsGetEndorsingRightsAction {
                    key: EndorsingRightsKey {
                        current_block_hash,
                        level: None,
                    },
                });
            }
        }
        Action::RightsEndorsingRightsReady(RightsEndorsingRightsReadyAction {
            key:
                EndorsingRightsKey {
                    current_block_hash,
                    level: None,
                },
            endorsing_rights,
        }) => {
            for key in prechecker_state_operations
                .iter()
                .filter_map(|(key, state)| {
                    if let PrecheckerOperationState::PendingEndorsingRights { block_hash, .. } =
                        state
                    {
                        if block_hash == current_block_hash {
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
                    if let PrecheckerOperationState::PendingEndorsingRights { block_hash, .. } =
                        state
                    {
                        if block_hash == current_block_hash {
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
                store.dispatch(PrecheckerErrorAction {
                    key,
                    error: error.clone().into(),
                });
            }
        }
        Action::PrecheckerEndorsingRightsReady(PrecheckerEndorsingRightsReadyAction {
            key,
            ..
        }) => {
            if let Some(PrecheckerOperationState::EndorsingRightsReady { .. }) =
                prechecker_state_operations.get(key)
            {
                store.dispatch(PrecheckerValidateEndorsementAction { key: key.clone() });
            }
        }
        Action::PrecheckerValidateEndorsement(PrecheckerValidateEndorsementAction { key }) => {
            if let Some(PrecheckerOperationState::PendingOperationPrechecking {
                block_hash,
                operation_binary_encoding,
                operation_decoded_contents,
                endorsing_rights,
            }) = prechecker_state_operations.get(key)
            {
                let chain_id = match store.state.get().prechecker.applied_blocks.get(block_hash) {
                    Some(AppliedBlockCache { chain_id, .. }) => chain_id,
                    None => {
                        error!(log, "!!! Missing chain id"; "block_hash" => block_hash.to_string());
                        return;
                    }
                };
                use super::EndorsementValidator;
                let validation_result = match operation_decoded_contents {
                    OperationDecodedContents::Proto010(operation) => operation
                        .validate_endorsement(
                            operation_binary_encoding,
                            chain_id,
                            endorsing_rights,
                            log,
                        ),
                };

                if let Err(err) = validation_result {
                    store.dispatch(PrecheckerErrorAction {
                        key: key.clone(),
                        error: err.into(),
                    });
                } else {
                    store.dispatch(PrecheckerEndorsementValidationReadyAction { key: key.clone() });
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
