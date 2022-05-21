// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::cmp::Ordering;

use crypto::{
    hash::{BlockHash, BlockPayloadHash, ChainId, OperationHash},
    PublicKeyWithHash,
};
use slog::Logger;
use tezos_messages::{p2p::encoding::block_header::Level, protocol::SupportedProtocol};

use crate::{rights::EndorsingRights, Action, ActionWithMeta, State};

use super::{
    prechecker_actions::*, ConsensusOperationError, EndorsementBranch, OperationDecodedContents,
    PrecheckerError, PrecheckerOperation, PrecheckerOperationState, Round,
    TenderbakeConsensusContents,
};

pub fn prechecker_reducer(state: &mut State, action: &ActionWithMeta) {
    // let PrecheckerState {
    //     cached_operations,
    //     operations,
    //     proto_cache,
    //     ..
    // } = &mut state.prechecker;
    let operations = &mut state.prechecker.operations;
    let cached_operations = &mut state.prechecker.cached_operations;
    let proto_cache = &mut state.prechecker.proto_cache;
    //let rights = &mut state.prechecker.rights;
    let rights = &mut state.rights;
    match &action.action {
        Action::PrecheckerCurrentHeadUpdate(PrecheckerCurrentHeadUpdateAction { head, .. }) => {
            let min_level = head.header.level().saturating_sub(2);
            let old_operations = cached_operations.remove_older(min_level);
            for old_operation in old_operations {
                let op_state = operations.remove(&old_operation);
                slog::debug!(
                    state.log,
                    "Removing old operation";
                    "state" => slog::FnValue(|_| format!("{:?}", op_state))
                );
            }
            operations.retain(|hash, operation| {
                let retain = if let Ok(op) = operation {
                    op.level().map_or(false, |level| level >= min_level)
                } else {
                    false
                };
                if !retain {
                    slog::debug!(
                        state.log,
                        "Pruning prececked operation {hash}";
                        "state" => slog::FnValue(|_| format!("{:?}", operation))
                    );
                }
                retain
            })
        }

        Action::PrecheckerStoreEndorsementBranch(PrecheckerStoreEndorsementBranchAction {
            endorsement_branch,
        }) => {
            state.prechecker.endorsement_branch = endorsement_branch.clone();
        }

        Action::PrecheckerPrecheckOperation(PrecheckerPrecheckOperationAction {
            operation,
            hash,
            proto,
        }) => {
            let protocol = if let Some(p) = proto_cache.get(proto) {
                p
            } else {
                slog::error!(state.log, "Undefined protocol for proto `{proto}`");
                return;
            };

            slog::debug!(state.log, "Prechecking operation `{hash}`");

            operations.insert(
                hash.clone(),
                Ok(PrecheckerOperation::new(
                    operation.clone(),
                    protocol.clone(),
                )),
            );
        }

        Action::PrecheckerRevalidateOperation(action) => {
            if let Some(Ok(PrecheckerOperation {
                state: op_state, ..
            })) = operations.get_mut(&action.hash)
            {
                let endorsing_rights_verified = match op_state {
                    PrecheckerOperationState::Applied { .. } => true,
                    PrecheckerOperationState::BranchDelayed {
                        endorsing_rights_verified,
                        ..
                    } => *endorsing_rights_verified,
                    _ => false,
                };
                match op_state {
                    PrecheckerOperationState::Applied {
                        operation_decoded_contents,
                    }
                    | PrecheckerOperationState::BranchRefused {
                        operation_decoded_contents,
                    }
                    | PrecheckerOperationState::BranchDelayed {
                        operation_decoded_contents,
                        ..
                    }
                    | PrecheckerOperationState::Outdated {
                        operation_decoded_contents,
                    } => {
                        if let Some(consensus_contents) =
                            operation_decoded_contents.as_tenderbake_consensus()
                        {
                            slog::debug!(state.log, "Revalidating operation `{}`", action.hash);
                            *op_state = PrecheckerOperationState::TenderbakeConsensus {
                                operation_decoded_contents: operation_decoded_contents.clone(),
                                consensus_contents,
                                endorsing_rights_verified,
                            };
                        }
                    }
                    _ => {}
                }
            }
        }

        Action::PrecheckerDecodeOperation(action) => {
            if let Some(Ok(PrecheckerOperation { operation, state })) =
                operations.get_mut(&action.hash)
            {
                if let PrecheckerOperationState::Init { protocol } = state {
                    *state = match OperationDecodedContents::parse(operation, protocol) {
                        Ok(operation_decoded_contents) => PrecheckerOperationState::Decoded {
                            operation_decoded_contents,
                        },
                        Err(error) => PrecheckerOperationState::Refused {
                            operation_decoded_contents: None,
                            error,
                        },
                    };
                }
            }
        }

        Action::PrecheckerCategorizeOperation(action) => {
            if let Some(Ok(PrecheckerOperation {
                operation: _,
                state: op_state,
            })) = operations.get_mut(&action.hash)
            {
                if let PrecheckerOperationState::Decoded {
                    operation_decoded_contents,
                } = op_state
                {
                    *op_state = if state.config.disable_endorsements_precheck {
                        PrecheckerOperationState::ProtocolNeeded
                    } else if let Some(consensus_contents) =
                        operation_decoded_contents.as_tenderbake_consensus()
                    {
                        PrecheckerOperationState::TenderbakeConsensus {
                            operation_decoded_contents: operation_decoded_contents.clone(),
                            consensus_contents,
                            endorsing_rights_verified: false,
                        }
                    } else {
                        PrecheckerOperationState::ProtocolNeeded
                    };
                }
            }
        }

        Action::PrecheckerValidateOperation(action) => {
            if let Some(Ok(PrecheckerOperation {
                state: op_state, ..
            })) = operations.get_mut(&action.hash)
            {
                let (operation_decoded_contents, consensus_contents, endorsing_rights_verified) =
                    match op_state {
                        PrecheckerOperationState::TenderbakeConsensus {
                            operation_decoded_contents,
                            consensus_contents,
                            endorsing_rights_verified,
                        }
                        | PrecheckerOperationState::TenderbakePendingRights {
                            operation_decoded_contents,
                            consensus_contents,
                            endorsing_rights_verified,
                        } => (
                            operation_decoded_contents,
                            consensus_contents,
                            endorsing_rights_verified,
                        ),
                        _ => {
                            *op_state = PrecheckerOperationState::ProtocolNeeded;
                            return;
                        }
                    };
                let tenderbake_validators =
                    if let Some(v) = rights.tenderbake_endorsing_rights(consensus_contents.level) {
                        v
                    } else {
                        *op_state = PrecheckerOperationState::TenderbakePendingRights {
                            operation_decoded_contents: operation_decoded_contents.clone(),
                            consensus_contents: consensus_contents.clone(),
                            endorsing_rights_verified: *endorsing_rights_verified,
                        };
                        return;
                    };

                let operation_branch = operation_decoded_contents.branch();
                let operation_decoded_contents = operation_decoded_contents.clone();

                if let Err(error_state) = validate_tenderbake_consensus_operation_contents(
                    state.prechecker.endorsement_branch.as_ref(),
                    consensus_contents,
                    operation_branch,
                ) {
                    slog::debug!(state.log, "Validation failed for `{}`", action.hash; "error" => slog::FnValue(|_| error_state.to_string()));
                    *op_state = if error_state.is_delayed() {
                        // check signature in advance
                        if *endorsing_rights_verified {
                            error_state
                                .into_prechecker_operation_state(operation_decoded_contents, true)
                        } else if let Err(err) = validate_tenderbake_consensus_operation_signature(
                            &action.hash,
                            &operation_decoded_contents,
                            consensus_contents,
                            tenderbake_validators,
                            &state.config.chain_id,
                            &state.log,
                        ) {
                            PrecheckerOperationState::Refused {
                                operation_decoded_contents: Some(operation_decoded_contents),
                                error: err.into(),
                            }
                        } else {
                            error_state
                                .into_prechecker_operation_state(operation_decoded_contents, true)
                        }
                    } else {
                        error_state.into_prechecker_operation_state(
                            operation_decoded_contents,
                            *endorsing_rights_verified,
                        )
                    };
                    return;
                }

                *op_state = if *endorsing_rights_verified {
                    PrecheckerOperationState::Applied {
                        operation_decoded_contents,
                    }
                } else if let Err(err) = validate_tenderbake_consensus_operation_signature(
                    &action.hash,
                    &operation_decoded_contents,
                    consensus_contents,
                    tenderbake_validators,
                    &state.config.chain_id,
                    &state.log,
                ) {
                    PrecheckerOperationState::Refused {
                        operation_decoded_contents: Some(operation_decoded_contents),
                        error: err.into(),
                    }
                } else {
                    PrecheckerOperationState::Applied {
                        operation_decoded_contents,
                    }
                }
            }
        }

        Action::PrecheckerCacheProtocol(PrecheckerCacheProtocolAction {
            proto,
            protocol_hash,
        }) => match SupportedProtocol::try_from(protocol_hash) {
            Ok(protocol) => {
                proto_cache.insert(*proto, protocol);
            }
            Err(err) => {
                slog::error!(
                    state.log,
                    "Failed to cache supported protocol `{proto}`: `{err}`"
                );
            }
        },

        Action::PrecheckerPruneOperation(PrecheckerPruneOperationAction { hash }) => {
            operations.remove(hash);
        }

        Action::PrecheckerCacheDelayedOperation(PrecheckerCacheDelayedOperationAction { hash }) => {
            if let Some(Ok(op)) = operations.get(hash) {
                if let Some(level) = op.state.caching_level() {
                    cached_operations.insert(level, hash.clone());
                }
            }
        }

        _ => (),
    }
}

#[derive(Debug, thiserror::Error)]
enum TenderbakeConsensusResult<'a> {
    #[error("level in the future, expected `{0}`, actual `{1}`")]
    LevelInFuture(Level, Level),
    #[error("level in the past, expected `{0}`, actual `{1}`")]
    LevelInPast(Level, Level),
    #[error("round in the future, expected `{0}`, actual `{1}`")]
    RoundInFuture(Round, Round),
    #[error("round in the past, expected `{0}`, actual `{1}`")]
    RoundInPast(Round, Round),
    #[error("wrong branch, expected `{0}`, actual `{1}`")]
    WrongBranch(&'a BlockHash, &'a BlockHash),
    #[error("competing proposal, expected `{0}`, actual `{1}`")]
    CompetingProposal(&'a BlockPayloadHash, &'a BlockPayloadHash),
}

impl<'a> TenderbakeConsensusResult<'a> {
    fn is_delayed(&self) -> bool {
        matches!(self, Self::LevelInFuture(..) | Self::RoundInFuture(..))
    }

    fn into_prechecker_operation_state(
        self,
        operation_decoded_contents: OperationDecodedContents,
        endorsing_rights_verified: bool,
    ) -> PrecheckerOperationState {
        match self {
            TenderbakeConsensusResult::LevelInFuture(..)
            | TenderbakeConsensusResult::RoundInFuture(..) => {
                PrecheckerOperationState::BranchDelayed {
                    operation_decoded_contents,
                    endorsing_rights_verified,
                }
            }
            TenderbakeConsensusResult::LevelInPast(..)
            | TenderbakeConsensusResult::RoundInPast(..) => PrecheckerOperationState::Outdated {
                operation_decoded_contents,
            },
            TenderbakeConsensusResult::WrongBranch(..) => PrecheckerOperationState::BranchRefused {
                operation_decoded_contents,
            },
            TenderbakeConsensusResult::CompetingProposal(..) => PrecheckerOperationState::Refused {
                operation_decoded_contents: Some(operation_decoded_contents),
                error: PrecheckerError::Consensus(ConsensusOperationError::CompetingProposal),
            },
        }
    }
}

fn validate_tenderbake_consensus_operation_contents<'a>(
    endorsement_branch: Option<&'a EndorsementBranch>,
    actual: &'a TenderbakeConsensusContents,
    actual_branch: &'a BlockHash,
) -> Result<(), TenderbakeConsensusResult<'a>> {
    let expected = endorsement_branch
        .ok_or_else(|| TenderbakeConsensusResult::LevelInFuture(-1, actual.level))?;

    match actual.level.cmp(&expected.level) {
        Ordering::Less => {
            return Err(TenderbakeConsensusResult::LevelInPast(
                expected.level,
                actual.level,
            ))
        }
        Ordering::Greater => {
            return Err(TenderbakeConsensusResult::LevelInFuture(
                expected.level,
                actual.level,
            ))
        }
        _ => (),
    }

    match actual.round.cmp(&expected.round) {
        Ordering::Less => {
            return Err(TenderbakeConsensusResult::RoundInPast(
                expected.round,
                actual.round,
            ))
        }
        Ordering::Greater => {
            return Err(TenderbakeConsensusResult::RoundInFuture(
                expected.round,
                actual.round,
            ))
        }
        _ => (),
    }

    if actual_branch != &expected.predecessor {
        return Err(TenderbakeConsensusResult::WrongBranch(
            &expected.predecessor,
            actual_branch,
        ));
    }

    if actual.payload_hash != expected.payload_hash {
        return Err(TenderbakeConsensusResult::CompetingProposal(
            &expected.payload_hash,
            &actual.payload_hash,
        ));
    }

    Ok(())
}

fn validate_tenderbake_consensus_operation_signature(
    hash: &OperationHash,
    operation_decoded_contents: &OperationDecodedContents,
    consensus_contents: &TenderbakeConsensusContents,
    tenderbake_validators: &EndorsingRights,
    chain_id: &ChainId,
    log: &Logger,
) -> Result<(), ConsensusOperationError> {
    let TenderbakeConsensusContents { slot, .. } = consensus_contents;
    let (delegate, _) = tenderbake_validators
        .slots
        .get(slot)
        .ok_or_else(|| ConsensusOperationError::IncorrectSlot(*slot))?;
    slog::debug!(log, "Delegate found for `{hash}`"; "delegate" => slog::FnValue(|_| delegate.pk_hash().map(|pkh| pkh.to_string_representation()).unwrap_or_default()));
    match operation_decoded_contents.verify_signature(delegate, chain_id) {
        Ok(true) => Ok(()),
        Ok(false) => Err(ConsensusOperationError::SignatureMismatch),
        Err(err) => Err(ConsensusOperationError::from(err)),
    }
}
