// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::convert::{TryFrom, TryInto};

use crypto::{blake2b, hash::BlockHash};
use slog::error;
use tezos_messages::{p2p::binary_message::BinaryWrite, protocol::SupportedProtocol};

use crate::{
    current_head_precheck::{
        CurrentHeadPrecheckRejectedAction, CurrentHeadPrecheckSuccessAction, CurrentHeadState,
    },
    mempool::mempool_actions::MempoolOperationDecodedAction,
    prechecker::{
        prechecker_actions::PrecheckerEndorsementValidationRefusedAction, Applied,
        PrecheckerOperationState, Refused,
    },
    rights::{rights_actions::*, RightsKey},
    storage::kv_block_additional_data::{
        StorageBlockAdditionalDataErrorAction, StorageBlockAdditionalDataGetAction,
        StorageBlockAdditionalDataOkAction,
    },
    Action, ActionWithMeta, Service, State, Store,
};

use super::{
    prechecker_actions::*, EndorsementValidationError, Key, OperationDecodedContents,
    PrecheckerError, PrecheckerOperation, SupportedProtocolState,
};

pub fn prechecker_effects<S>(store: &mut Store<S>, action: &ActionWithMeta)
where
    S: Service,
{
    let prechecker_state = &store.state.get().prechecker;
    let prechecker_state_operations = &prechecker_state.operations;
    let log = &store.state.get().log;
    match &action.action {
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
                    store.dispatch(PrecheckerGetProtocolVersionAction { key: key.clone() });
                }
                Some(PrecheckerOperationState::Applied { .. }) => {
                    let action = PrecheckerEndorsementValidationAppliedAction { key: key.clone() };
                    store.dispatch(action);
                }
                Some(PrecheckerOperationState::Error { error, .. }) => {
                    let error = error.clone();
                    store.dispatch(PrecheckerErrorAction::new(key.clone(), error));
                }
                _ => (),
            };
        }
        Action::PrecheckerGetProtocolVersion(PrecheckerGetProtocolVersionAction { key }) => {
            if let Some((_, SupportedProtocolState::Ready(_))) =
                store.state.get().prechecker.next_protocol
            {
                store.dispatch(PrecheckerProtocolVersionReadyAction { key: key.clone() });
            }
            // otherwise `BlockApplied` will trigger protocol version fetching
        }
        Action::PrecheckerProtocolVersionReady(PrecheckerProtocolVersionReadyAction { key }) => {
            store.dispatch(PrecheckerDecodeOperationAction { key: key.clone() });
        }
        Action::PrecheckerDecodeOperation(PrecheckerDecodeOperationAction { key }) => {
            let proto = match &prechecker_state.next_protocol {
                Some((_, SupportedProtocolState::Ready(proto))) => proto,
                _ => {
                    store.dispatch(PrecheckerGetProtocolVersionAction { key: key.clone() });
                    return;
                }
            };
            if let Some(PrecheckerOperation {
                operation_binary_encoding,
                state: PrecheckerOperationState::PendingContentDecoding,
                ..
            }) = prechecker_state_operations.get(key)
            {
                // TODO use proper protocol to parse operation
                match OperationDecodedContents::parse(operation_binary_encoding, proto) {
                    Ok(contents) => {
                        store.dispatch(PrecheckerOperationDecodedAction {
                            key: key.clone(),
                            contents,
                        });
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
                let is_endorsement = operation_decoded_contents.is_endorsement();
                let endorsement_level = operation_decoded_contents.endorsement_level();
                let block = operation_decoded_contents.branch().clone();
                let operation_decoded_contents = operation_decoded_contents.clone();
                let disable_block_precheck = store.state().config.disable_block_precheck;
                let disable_endorsements_precheck =
                    store.state().config.disable_endorsements_precheck;

                store.dispatch(MempoolOperationDecodedAction {
                    operation: key.operation.clone(),
                    operation_decoded_contents,
                });

                if disable_endorsements_precheck || !is_endorsement {
                    store.dispatch(PrecheckerProtocolNeededAction { key: key.clone() });
                } else if disable_block_precheck {
                    let current_head = match store.state().current_head.get() {
                        Some(v) => v,
                        None => return,
                    };
                    if current_head.hash == block {
                        store.dispatch(PrecheckerGetEndorsingRightsAction { key: key.clone() });
                    } else if Some(current_head.header.level() + 1) == endorsement_level {
                        store.dispatch(PrecheckerWaitForBlockAppliedAction {
                            key: key.clone(),
                            branch: block,
                        });
                    }
                } else if store.state.get().current_head_level() == endorsement_level {
                    store.dispatch(PrecheckerWaitForBlockPrecheckedAction {
                        key: key.clone(),
                        branch: block,
                    });
                }
            }
        }
        Action::PrecheckerWaitForBlockPrechecked(PrecheckerWaitForBlockPrecheckedAction {
            key,
            branch,
        }) => {
            if let Some(PrecheckerOperationState::PendingBlockPrechecked { .. }) =
                prechecker_state_operations.get(key).map(|op| &op.state)
            {
                match endorsement_branch_is_valid(store.state.get(), branch) {
                    Some(true) => {
                        store.dispatch(PrecheckerBlockPrecheckedAction { key: key.clone() });
                    }
                    Some(false) => {
                        store.dispatch(PrecheckerEndorsementValidationRefusedAction {
                            key: key.clone(),
                            error: EndorsementValidationError::InvalidBranch,
                        });
                    }
                    None => {
                        slog::trace!(&store.state.get().log, "=== prechecker cannot decide on `{op}` as `{branch}` is not yet prechecked", op = key.operation.to_base58_check());
                    }
                }
            }
        }
        Action::CurrentHeadPrecheckSuccess(CurrentHeadPrecheckSuccessAction {
            block_hash, ..
        }) => {
            for key in store
                .state
                .get()
                .prechecker
                .operations
                .iter()
                .filter_map(|(k, v)| {
                    if let PrecheckerOperationState::PendingBlockPrechecked {
                        operation_decoded_contents,
                    } = &v.state
                    {
                        if operation_decoded_contents.branch() == block_hash {
                            Some(k.clone())
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
            {
                store.dispatch(PrecheckerBlockPrecheckedAction { key });
            }
        }
        Action::CurrentHeadPrecheckRejected(CurrentHeadPrecheckRejectedAction {
            block_hash,
            ..
        }) => {
            for key in store
                .state
                .get()
                .prechecker
                .operations
                .iter()
                .filter_map(|(k, v)| {
                    if let PrecheckerOperationState::PendingBlockPrechecked {
                        operation_decoded_contents,
                    } = &v.state
                    {
                        if operation_decoded_contents.branch() == block_hash {
                            Some(k.clone())
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
            {
                store.dispatch(PrecheckerWaitForBlockAppliedAction {
                    key,
                    branch: block_hash.clone(),
                });
            }
        }
        Action::PrecheckerBlockPrechecked(PrecheckerBlockPrecheckedAction { key })
        | Action::PrecheckerBlockApplied(PrecheckerBlockAppliedAction { key }) => {
            store.dispatch(PrecheckerGetEndorsingRightsAction { key: key.clone() });
        }
        Action::PrecheckerGetEndorsingRights(PrecheckerGetEndorsingRightsAction { key }) => {
            if let Some(PrecheckerOperationState::PendingEndorsingRights {
                operation_decoded_contents,
                ..
            }) = prechecker_state_operations.get(key).map(|op| &op.state)
            {
                if let Some(current_block_hash) = store
                    .state
                    .get()
                    .mempool
                    .local_head_state
                    .as_ref()
                    .map(|lhs| lhs.hash.clone())
                {
                    if let Some(level) = operation_decoded_contents.endorsement_level() {
                        store.dispatch(RightsGetAction {
                            key: RightsKey::endorsing(current_block_hash, Some(level)),
                        });
                    }
                }
            }
        }
        Action::RightsEndorsingReady(RightsEndorsingReadyAction {
            key,
            endorsing_rights,
        }) => {
            let level = if let Some(level) = key.level() {
                level
            } else {
                return;
            };
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
                            .map(|l| l == level)
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
        Action::RightsError(RightsErrorAction { key, error }) => {
            let level = if let Some(level) = key.level() {
                level
            } else {
                return;
            };
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
                            .map(|l| l == level)
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
                state:
                    PrecheckerOperationState::PendingOperationPrechecking {
                        operation_decoded_contents,
                        endorsing_rights,
                    },
                ..
            }) = prechecker_state_operations.get(key)
            {
                let block_hash = operation_decoded_contents.branch();
                let chain_id = &store.state.get().config.chain_id;
                use super::EndorsementValidator;
                let validation_result = operation_decoded_contents.validate_endorsement(
                    chain_id,
                    block_hash,
                    endorsing_rights,
                    log,
                );

                match validation_result {
                    Ok(Applied { .. }) => {
                        store.dispatch(PrecheckerEndorsementValidationAppliedAction {
                            key: key.clone(),
                        });
                    }
                    Err(Refused { error, .. }) => {
                        store.dispatch(PrecheckerEndorsementValidationRefusedAction {
                            key: key.clone(),
                            error,
                        });
                    }
                };
            }
        }
        Action::PrecheckerProtocolNeeded(PrecheckerProtocolNeededAction { key, .. }) => {
            if let Some(PrecheckerOperationState::ProtocolNeeded { .. }) =
                prechecker_state_operations.get(key).map(|op| &op.state)
            {
                store.dispatch(PrecheckerPrecheckOperationResponseAction::prevalidate(
                    &key.operation,
                ));
            }
            store.dispatch(PrecheckerPruneOperationAction { key: key.clone() });
        }

        Action::PrecheckerEndorsementValidationApplied(
            PrecheckerEndorsementValidationAppliedAction { key },
        ) => {
            if let Some(PrecheckerOperationState::Applied {
                operation_decoded_contents,
            }) = prechecker_state_operations.get(key).map(|op| &op.state)
            {
                let operation_decoded_contents = operation_decoded_contents.clone();
                store.dispatch(PrecheckerPrecheckOperationResponseAction::valid(
                    &key.operation,
                    operation_decoded_contents,
                ));
            }
            store.dispatch(PrecheckerPruneOperationAction { key: key.clone() });
        }
        Action::PrecheckerEndorsementValidationRefused(
            PrecheckerEndorsementValidationRefusedAction { key, .. },
        ) => {
            if let Some(PrecheckerOperation {
                state:
                    PrecheckerOperationState::Refused {
                        operation_decoded_contents,
                        error,
                    },
                ..
            }) = prechecker_state_operations.get(key)
            {
                let action = PrecheckerPrecheckOperationResponseAction::reject(
                    &key.operation,
                    operation_decoded_contents.clone(),
                    serde_json::to_string(error).unwrap_or_else(|_| "<unserialized>".to_string()),
                );
                store.dispatch(action);
            }
            store.dispatch(PrecheckerPruneOperationAction { key: key.clone() });
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
                    PrecheckerError::Protocol(_) => {
                        store.dispatch(PrecheckerPrecheckOperationResponseAction::prevalidate(
                            &key.operation,
                        ));
                    }
                    PrecheckerError::Storage(err) => {
                        store.dispatch(PrecheckerPrecheckOperationResponseAction::error(
                            err.clone(),
                        ));
                    }
                }
            }
        }

        // precache endorsing rights for the next level
        Action::PrecheckerPrecacheEndorsingRights(PrecheckerPrecacheEndorsingRightsAction {
            current_head,
            level,
        }) => {
            slog::trace!(&store.state.get().log, "precaching endorsing rights"; "level" => level + 1);
            store.dispatch(RightsGetAction {
                key: RightsKey::endorsing(current_head.clone(), Some(level + 1)),
            });
        }
        // prechache next block protocol, applied block is needed for that
        Action::CurrentHeadUpdate(content) => {
            let block = &content.new_head;

            let need_proto_update =
                prechecker_state
                    .next_protocol
                    .as_ref()
                    .map_or(true, |(proto, state)| {
                        (*proto == block.header.proto()
                            && matches!(state, SupportedProtocolState::None))
                            || proto + 1 == block.header.proto()
                    });

            let (block_hash, proto) = (block.hash.clone(), block.header.proto());
            for key in store
                .state
                .get()
                .prechecker
                .operations
                .iter()
                .filter_map(|(k, v)| {
                    if let PrecheckerOperationState::PendingBlockApplied {
                        operation_decoded_contents,
                    } = &v.state
                    {
                        if operation_decoded_contents.branch() == &block_hash {
                            Some(k.clone())
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
            {
                store.dispatch(PrecheckerBlockAppliedAction { key });
            }

            if need_proto_update {
                slog::trace!(&store.state.get().log, "query next block protocol"; "proto" => proto, "block" => block_hash.to_base58_check());
                store.dispatch(PrecheckerQueryNextBlockProtocolAction { block_hash, proto });
            }
        }
        Action::PrecheckerQueryNextBlockProtocol(PrecheckerQueryNextBlockProtocolAction {
            block_hash,
            ..
        }) => {
            store.dispatch(StorageBlockAdditionalDataGetAction {
                key: block_hash.clone(),
            });
        }
        Action::StorageBlockAdditionalDataOk(StorageBlockAdditionalDataOkAction { key, value }) => {
            match SupportedProtocol::try_from(value.next_protocol_hash()) {
                Ok(proto) => {
                    store.dispatch(PrecheckerNextBlockProtocolReadyAction {
                        block_hash: key.clone(),
                        supported_protocol: proto,
                    });
                }
                Err(err) => {
                    store.dispatch(PrecheckerNextBlockProtocolErrorAction {
                        block_hash: key.clone(),
                        error: err.into(),
                    });
                }
            };
        }
        Action::StorageBlockAdditionalDataError(StorageBlockAdditionalDataErrorAction {
            key,
            error,
        }) => {
            store.dispatch(PrecheckerNextBlockProtocolErrorAction {
                block_hash: key.clone(),
                error: error.clone().into(),
            });
        }

        Action::PrecheckerNextBlockProtocolReady(PrecheckerNextBlockProtocolReadyAction {
            ..
        }) => {
            for key in store
                .state
                .get()
                .prechecker
                .operations
                .iter()
                .filter_map(|(k, op)| {
                    if let PrecheckerOperationState::PendingProtocolVersion = op.state {
                        Some(k.clone())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
            {
                store.dispatch(PrecheckerProtocolVersionReadyAction { key });
            }
        }
        Action::PrecheckerNextBlockProtocolError(PrecheckerNextBlockProtocolErrorAction {
            error,
            ..
        }) => {
            for key in store
                .state
                .get()
                .prechecker
                .operations
                .iter()
                .filter_map(|(k, op)| {
                    if let PrecheckerOperationState::PendingProtocolVersion = op.state {
                        Some(k.clone())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
            {
                store.dispatch(PrecheckerErrorAction {
                    key,
                    error: error.clone(),
                });
            }
        }

        _ => (),
    }
}

fn endorsement_branch_is_valid(state: &State, branch: &BlockHash) -> Option<bool> {
    if state.current_heads.candidates.is_empty() {
        state
            .current_head_hash()
            .map(|head_hash| head_hash == branch)
    } else {
        match state.current_heads.candidates.get(branch) {
            Some(CurrentHeadState::Prechecked { .. }) => Some(true),
            Some(CurrentHeadState::Rejected { .. }) | None => Some(false),
            _ => None,
        }
    }
}
