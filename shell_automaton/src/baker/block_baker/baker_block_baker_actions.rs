// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use tezos_messages::base::signature_public_key::SignaturePublicKey;

use crate::baker::MINIMAL_BLOCK_DELAY;
use crate::{EnablingCondition, State};

use super::BakerBlockBakerState;

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
                BakerBlockBakerState::TimeoutPending { next_level, .. } => {
                    next_level.map_or(false, |v| v.timeout <= state.time_as_nanos())
                        && baker.elected_block.is_none()
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
