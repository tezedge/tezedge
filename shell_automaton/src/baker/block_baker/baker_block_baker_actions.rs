// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use crypto::hash::{NonceHash, Signature};
use storage::BlockHeaderWithHash;
use tezos_encoding::types::SizedBytes;
use tezos_messages::base::signature_public_key::SignaturePublicKey;
use tezos_messages::p2p::encoding::operations_for_blocks::Path;

use crate::baker::MINIMAL_BLOCK_DELAY;
use crate::protocol_runner::ProtocolRunnerToken;
use crate::request::RequestId;
use crate::{EnablingCondition, State};

use super::{BakerBlockBakerState, BlockPreapplyRequest, BlockPreapplyResponse};

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerBlockBakerRightsGetInitAction {}

impl EnablingCondition<State> for BakerBlockBakerRightsGetInitAction {
    fn is_enabled(&self, state: &State) -> bool {
        state.is_bootstrapped()
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerBlockBakerRightsGetPendingAction {
    pub baker: SignaturePublicKey,
}

impl EnablingCondition<State> for BakerBlockBakerRightsGetPendingAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .bakers
            .get(&self.baker)
            .map_or(false, |baker| baker.block_baker.is_idle())
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerBlockBakerRightsGetCurrentLevelSuccessAction {
    pub baker: SignaturePublicKey,
    pub slots: Vec<u16>,
}

impl EnablingCondition<State> for BakerBlockBakerRightsGetCurrentLevelSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .bakers
            .get(&self.baker)
            .map_or(false, |baker| match &baker.block_baker {
                BakerBlockBakerState::RightsGetPending { slots, .. } => slots.is_none(),
                _ => false,
            })
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerBlockBakerRightsGetNextLevelSuccessAction {
    pub baker: SignaturePublicKey,
    pub slots: Vec<u16>,
}

impl EnablingCondition<State> for BakerBlockBakerRightsGetNextLevelSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .bakers
            .get(&self.baker)
            .map_or(false, |baker| match &baker.block_baker {
                BakerBlockBakerState::RightsGetPending { next_slots, .. } => next_slots.is_none(),
                _ => false,
            })
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerBlockBakerRightsGetSuccessAction {
    pub baker: SignaturePublicKey,
}

impl EnablingCondition<State> for BakerBlockBakerRightsGetSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .bakers
            .get(&self.baker)
            .map_or(false, |baker| match &baker.block_baker {
                BakerBlockBakerState::RightsGetPending {
                    slots, next_slots, ..
                } => match (slots, next_slots) {
                    (Some(slots), Some(next_slots)) => !slots.is_empty() || !next_slots.is_empty(),
                    _ => false,
                },
                _ => false,
            })
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerBlockBakerRightsNoRightsAction {
    pub baker: SignaturePublicKey,
}

impl EnablingCondition<State> for BakerBlockBakerRightsNoRightsAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .bakers
            .get(&self.baker)
            .map_or(false, |baker| match &baker.block_baker {
                BakerBlockBakerState::RightsGetPending {
                    slots, next_slots, ..
                } => {
                    slots.as_ref().map_or(false, |v| v.is_empty())
                        && next_slots.as_ref().map_or(false, |v| v.is_empty())
                }
                _ => false,
            })
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerBlockBakerTimeoutPendingAction {
    pub baker: SignaturePublicKey,
}

impl EnablingCondition<State> for BakerBlockBakerTimeoutPendingAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .bakers
            .get(&self.baker)
            .map_or(false, |baker| match &baker.block_baker {
                BakerBlockBakerState::RightsGetSuccess { .. } => true,
                _ => false,
            })
    }
}

/// Noop Action.
///
/// Doesn't cause state change or side-effects. Only useful for tracing.
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerBlockBakerNextLevelTimeoutSuccessQuorumPendingAction {
    pub baker: SignaturePublicKey,
}

impl EnablingCondition<State> for BakerBlockBakerNextLevelTimeoutSuccessQuorumPendingAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .bakers
            .get(&self.baker)
            .map_or(false, |baker| match &baker.block_baker {
                BakerBlockBakerState::TimeoutPending {
                    next_level,
                    next_level_timeout_notified,
                    ..
                } => {
                    next_level.map_or(false, |v| v.timeout <= state.time_as_nanos())
                        && baker.elected_block.is_none()
                        && !next_level_timeout_notified
                }
                _ => false,
            })
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerBlockBakerBakeNextLevelAction {
    pub baker: SignaturePublicKey,
}

impl EnablingCondition<State> for BakerBlockBakerBakeNextLevelAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .bakers
            .get(&self.baker)
            .map_or(false, |baker| match &baker.block_baker {
                BakerBlockBakerState::TimeoutPending { next_level, .. } => {
                    next_level.map_or(false, |v| v.timeout <= state.time_as_nanos())
                        && baker.elected_block.is_some()
                }
                _ => false,
            })
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerBlockBakerBakeNextRoundAction {
    pub baker: SignaturePublicKey,
}

impl EnablingCondition<State> for BakerBlockBakerBakeNextRoundAction {
    fn is_enabled(&self, state: &State) -> bool {
        state.bakers.get(&self.baker).map_or(false, |baker| {
            match &baker.block_baker {
                BakerBlockBakerState::TimeoutPending {
                    next_level,
                    next_round,
                    ..
                } => {
                    let now = state.time_as_nanos();
                    let has_elected_block = baker.elected_block.is_some();
                    !next_level.map_or(false, |v| v.timeout <= now && has_elected_block)
                        && next_round.map_or(false, |v| match has_elected_block {
                            false => v.timeout <= now,
                            true => {
                                // add a delay when quorum has been reached.
                                let delay = (MINIMAL_BLOCK_DELAY * 1_000_000_000) / 5;
                                v.timeout + delay <= now
                            }
                        })
                }
                _ => false,
            }
        })
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerBlockBakerBuildBlockInitAction {
    pub baker: SignaturePublicKey,
}

impl EnablingCondition<State> for BakerBlockBakerBuildBlockInitAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .bakers
            .get(&self.baker)
            .map_or(false, |baker| match &baker.block_baker {
                BakerBlockBakerState::BakeNextLevel { .. }
                | BakerBlockBakerState::BakeNextRound { .. } => true,
                _ => false,
            })
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerBlockBakerBuildBlockSuccessAction {
    pub baker: SignaturePublicKey,
    pub seed_nonce_hash: Option<NonceHash>,
}

impl EnablingCondition<State> for BakerBlockBakerBuildBlockSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .bakers
            .get(&self.baker)
            .map_or(false, |baker| match &baker.block_baker {
                BakerBlockBakerState::BakeNextLevel { .. }
                | BakerBlockBakerState::BakeNextRound { .. } => true,
                _ => false,
            })
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerBlockBakerPreapplyInitAction {
    pub baker: SignaturePublicKey,
}

impl EnablingCondition<State> for BakerBlockBakerPreapplyInitAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .bakers
            .get(&self.baker)
            .map_or(false, |baker| match &baker.block_baker {
                BakerBlockBakerState::BuildBlock { .. } => true,
                _ => false,
            })
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerBlockBakerPreapplyPendingAction {
    pub baker: SignaturePublicKey,
    pub protocol_req_id: ProtocolRunnerToken,
    pub request: BlockPreapplyRequest,
}

impl EnablingCondition<State> for BakerBlockBakerPreapplyPendingAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .bakers
            .get(&self.baker)
            .map_or(false, |baker| match &baker.block_baker {
                BakerBlockBakerState::BuildBlock { .. } => true,
                _ => false,
            })
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerBlockBakerPreapplySuccessAction {
    pub baker: SignaturePublicKey,
    pub response: BlockPreapplyResponse,
}

impl EnablingCondition<State> for BakerBlockBakerPreapplySuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .bakers
            .get(&self.baker)
            .map_or(false, |baker| match &baker.block_baker {
                BakerBlockBakerState::PreapplyPending { .. } => true,
                _ => false,
            })
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerBlockBakerComputeProofOfWorkInitAction {
    pub baker: SignaturePublicKey,
}

impl EnablingCondition<State> for BakerBlockBakerComputeProofOfWorkInitAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .bakers
            .get(&self.baker)
            .map_or(false, |baker| match &baker.block_baker {
                BakerBlockBakerState::PreapplySuccess { .. } => true,
                _ => false,
            })
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerBlockBakerComputeProofOfWorkPendingAction {
    pub baker: SignaturePublicKey,
    pub req_id: RequestId,
}

impl EnablingCondition<State> for BakerBlockBakerComputeProofOfWorkPendingAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .bakers
            .get(&self.baker)
            .map_or(false, |baker| match &baker.block_baker {
                BakerBlockBakerState::PreapplySuccess { .. } => true,
                _ => false,
            })
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerBlockBakerComputeProofOfWorkSuccessAction {
    pub baker: SignaturePublicKey,
    pub proof_of_work_nonce: SizedBytes<8>,
}

impl EnablingCondition<State> for BakerBlockBakerComputeProofOfWorkSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .bakers
            .get(&self.baker)
            .map_or(false, |baker| match &baker.block_baker {
                BakerBlockBakerState::ComputeProofOfWorkPending { .. } => true,
                _ => false,
            })
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerBlockBakerSignPendingAction {
    pub baker: SignaturePublicKey,
}

impl EnablingCondition<State> for BakerBlockBakerSignPendingAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .bakers
            .get(&self.baker)
            .map_or(false, |baker| match &baker.block_baker {
                BakerBlockBakerState::ComputeProofOfWorkSuccess { .. } => true,
                _ => false,
            })
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerBlockBakerSignSuccessAction {
    pub baker: SignaturePublicKey,
    pub signature: Signature,
}

impl EnablingCondition<State> for BakerBlockBakerSignSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .bakers
            .get(&self.baker)
            .map_or(false, |baker| match &baker.block_baker {
                BakerBlockBakerState::SignPending { .. } => true,
                _ => false,
            })
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerBlockBakerComputeOperationsPathsInitAction {
    pub baker: SignaturePublicKey,
}

impl EnablingCondition<State> for BakerBlockBakerComputeOperationsPathsInitAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .bakers
            .get(&self.baker)
            .map_or(false, |baker| match &baker.block_baker {
                BakerBlockBakerState::SignSuccess { .. } => true,
                _ => false,
            })
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerBlockBakerComputeOperationsPathsPendingAction {
    pub baker: SignaturePublicKey,
    pub protocol_req_id: ProtocolRunnerToken,
}

impl EnablingCondition<State> for BakerBlockBakerComputeOperationsPathsPendingAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .bakers
            .get(&self.baker)
            .map_or(false, |baker| match &baker.block_baker {
                BakerBlockBakerState::SignSuccess { .. } => true,
                _ => false,
            })
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerBlockBakerComputeOperationsPathsSuccessAction {
    pub baker: SignaturePublicKey,
    pub operations_paths: Vec<Path>,
}

impl EnablingCondition<State> for BakerBlockBakerComputeOperationsPathsSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .bakers
            .get(&self.baker)
            .map_or(false, |baker| match &baker.block_baker {
                BakerBlockBakerState::ComputeOperationsPathsPending { .. } => true,
                _ => false,
            })
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerBlockBakerInjectInitAction {
    pub baker: SignaturePublicKey,
}

impl EnablingCondition<State> for BakerBlockBakerInjectInitAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .bakers
            .get(&self.baker)
            .map_or(false, |baker| match &baker.block_baker {
                BakerBlockBakerState::ComputeOperationsPathsSuccess { .. } => true,
                _ => false,
            })
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerBlockBakerInjectPendingAction {
    pub baker: SignaturePublicKey,
    pub block: BlockHeaderWithHash,
}

impl EnablingCondition<State> for BakerBlockBakerInjectPendingAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .bakers
            .get(&self.baker)
            .map_or(false, |baker| match &baker.block_baker {
                BakerBlockBakerState::ComputeOperationsPathsSuccess { .. } => true,
                _ => false,
            })
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerBlockBakerInjectSuccessAction {
    pub baker: SignaturePublicKey,
}

impl EnablingCondition<State> for BakerBlockBakerInjectSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .bakers
            .get(&self.baker)
            .map_or(false, |baker| match &baker.block_baker {
                BakerBlockBakerState::InjectPending { .. } => true,
                _ => false,
            })
    }
}
