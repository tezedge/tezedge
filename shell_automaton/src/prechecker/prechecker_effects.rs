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
    Action, ActionWithMeta, Service, State, Store,
};

use super::{
    prechecker_actions::*, protocol_for_block, EndorsementValidationError, Key,
    OperationDecodedContents, PrecheckerError, PrecheckerOperation,
};

pub fn prechecker_effects<S>(store: &mut Store<S>, action: &ActionWithMeta)
where
    S: Service,
{
    let prechecker_state = &store.state.get().prechecker;
    let prechecker_state_operations = &prechecker_state.operations;
    let log = &store.state.get().log;
    match &action.action {
        Action::PrecheckerPrecheckBlock(PrecheckerPrecheckBlockAction {
            block_hash,
            block_header,
        }) => {
            if let Some(protocol) = protocol_for_block(block_header, prechecker_state) {
                match protocol {
                    SupportedProtocol::Proto012 => {
                        if let Ok(proto_header) =
                            tezos_messages::protocol::proto_012::block_header::BlockHeader::try_from(
                                block_header,
                            )
                        {
                            if let Some(block_stats) = store
                                .service
                                .statistics()
                                .and_then(|stats| stats.get_mut(block_hash))
                            {
                                block_stats.payload_hash = Some(proto_header.payload_hash.clone());
                                block_stats.payload_round = Some(proto_header.payload_round);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        Action::PrecheckerPrecheckOperationRequest(PrecheckerPrecheckOperationRequestAction {
            operation,
        }) => {
            let binary_encoding = match operation.as_bytes() {
                Ok(bytes) => bytes,
                Err(err) => {
                    store.dispatch(PrecheckerPrecheckOperationResponseAction::error(err, None));
                    return;
                }
            };

            let hash = match blake2b::digest_256(&binary_encoding) {
                Ok(hash) => hash,
                Err(err) => {
                    store.dispatch(PrecheckerPrecheckOperationResponseAction::error(err, None));
                    return;
                }
            };
            let key = match hash.try_into() {
                Ok(hash) => Key { operation: hash },
                Err(err) => {
                    store.dispatch(PrecheckerPrecheckOperationResponseAction::error(err, None));
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
            if let Some(operation) = prechecker_state_operations.get(key) {
                match &operation.state {
                    PrecheckerOperationState::Init { .. } => {
                        let header = if let Some((_, h)) = prechecker_state
                            .blocks_cache
                            .get(operation.operation.branch())
                        {
                            h
                        } else {
                            let block_hash = operation.operation.branch().clone();
                            store.dispatch(PrecheckerErrorAction::new(
                                key.clone(),
                                PrecheckerError::MissingBlockHeader(block_hash),
                            ));
                            return;
                        };
                        let protocol =
                            if let Some(p) = prechecker_state.proto_cache.get(&header.proto()) {
                                p
                            } else if let Some((_, _, p)) =
                                prechecker_state.protocol_cache.get(header.predecessor())
                            {
                                p
                            } else {
                                let block_hash = operation.operation.branch().clone();
                                store.dispatch(PrecheckerErrorAction::new(
                                    key.clone(),
                                    PrecheckerError::MissingProtocol(block_hash),
                                ));
                                return;
                            };
                        let protocol = match SupportedProtocol::try_from(protocol) {
                            Ok(p) => p,
                            Err(err) => {
                                store.dispatch(PrecheckerErrorAction::new(key.clone(), err));
                                return;
                            }
                        };
                        store.dispatch(PrecheckerDecodeOperationAction {
                            key: key.clone(),
                            protocol: protocol.clone(),
                        });
                    }
                    PrecheckerOperationState::Applied { .. } => {
                        let action =
                            PrecheckerEndorsementValidationAppliedAction { key: key.clone() };
                        store.dispatch(action);
                    }
                    PrecheckerOperationState::Error { error, .. } => {
                        let error = error.clone();
                        store.dispatch(PrecheckerErrorAction::new(key.clone(), error));
                    }
                    _ => (),
                }
            };
        }
        Action::PrecheckerDecodeOperation(PrecheckerDecodeOperationAction { key, protocol }) => {
            if let Some(PrecheckerOperation {
                operation_binary_encoding,
                state: PrecheckerOperationState::PendingContentDecoding,
                ..
            }) = prechecker_state_operations.get(key)
            {
                // TODO use proper protocol to parse operation
                match OperationDecodedContents::parse(operation_binary_encoding, protocol) {
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
                let ithaca_protocol = matches!(
                    operation_decoded_contents,
                    OperationDecodedContents::Proto012(_)
                );

                store.dispatch(MempoolOperationDecodedAction {
                    operation: key.operation.clone(),
                    operation_decoded_contents,
                });

                if disable_endorsements_precheck || !is_endorsement || ithaca_protocol {
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
            if let Some(op) = prechecker_state_operations.get(key) {
                if matches!(op.state, PrecheckerOperationState::ProtocolNeeded {}) {
                    let operation = op.operation.clone();
                    store.dispatch(PrecheckerPrecheckOperationResponseAction::prevalidate(
                        operation,
                        key.operation.clone(),
                    ));
                }
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
            if let Some(op) = prechecker_state_operations.get(key) {
                if matches!(op.state, PrecheckerOperationState::Error { .. }) {
                    match error {
                        PrecheckerError::EndorsingRights(err) => {
                            error!(log, "Getting endorsing rights failed"; "operation" => key.to_string(), "error" => err.to_string());
                            store.dispatch(PrecheckerPrecheckOperationResponseAction::error(
                                err.clone(),
                                Some(key.operation.clone()),
                            ));
                        }
                        PrecheckerError::UnsupportedProtocol(_) => {
                            let operation = op.operation.clone();
                            store.dispatch(PrecheckerPrecheckOperationResponseAction::prevalidate(
                                operation,
                                key.operation.clone(),
                            ));
                        }

                        PrecheckerError::Storage(err) => {
                            store.dispatch(PrecheckerPrecheckOperationResponseAction::error(
                                err.clone(),
                                Some(key.operation.clone()),
                            ));
                        }
                        PrecheckerError::MissingBlockHeader(_)
                        | PrecheckerError::MissingProtocol(_)
                        | PrecheckerError::OperationContentsDecode(_) => {
                            store.dispatch(PrecheckerPrecheckOperationResponseAction::error(
                                error.clone(),
                                Some(key.operation.clone()),
                            ));
                        }
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

            let (block_hash, _proto) = (block.hash.clone(), block.header.proto());
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
            store.dispatch(PrecheckerCacheProtocolAction {
                block_hash: content.new_head.hash.clone(),
                proto: content.new_head.header.proto(),
                protocol_hash: content.protocol.clone(),
                next_protocol_hash: content.next_protocol.clone(),
            });
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
