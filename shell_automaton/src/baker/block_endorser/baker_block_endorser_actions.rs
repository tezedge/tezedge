// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use crypto::hash::Signature;
use tezos_messages::base::signature_public_key::SignaturePublicKey;

use crate::rights::EndorsingPower;
use crate::{EnablingCondition, State};

use super::{BakerBlockEndorserState, EndorsementWithForgedBytes, PreendorsementWithForgedBytes};

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerBlockEndorserRightsGetInitAction {}

impl EnablingCondition<State> for BakerBlockEndorserRightsGetInitAction {
    fn is_enabled(&self, state: &State) -> bool {
        state.is_bootstrapped()
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerBlockEndorserRightsGetPendingAction {
    pub baker: SignaturePublicKey,
}

impl EnablingCondition<State> for BakerBlockEndorserRightsGetPendingAction {
    fn is_enabled(&self, state: &State) -> bool {
        state.bakers.get(&self.baker).map_or(false, |baker| {
            matches!(baker.block_endorser, BakerBlockEndorserState::Idle { .. })
        })
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerBlockEndorserRightsGetSuccessAction {
    pub baker: SignaturePublicKey,
    pub first_slot: u16,
    pub endorsing_power: EndorsingPower,
}

impl EnablingCondition<State> for BakerBlockEndorserRightsGetSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        state.bakers.get(&self.baker).map_or(false, |baker| {
            matches!(
                baker.block_endorser,
                BakerBlockEndorserState::RightsGetPending { .. }
            )
        })
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerBlockEndorserRightsNoRightsAction {
    pub baker: SignaturePublicKey,
}

impl EnablingCondition<State> for BakerBlockEndorserRightsNoRightsAction {
    fn is_enabled(&self, state: &State) -> bool {
        state.bakers.get(&self.baker).map_or(false, |baker| {
            matches!(
                baker.block_endorser,
                BakerBlockEndorserState::RightsGetPending { .. }
            )
        })
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerBlockEndorserPreendorseAction {
    pub baker: SignaturePublicKey,
}

impl EnablingCondition<State> for BakerBlockEndorserPreendorseAction {
    fn is_enabled(&self, state: &State) -> bool {
        state.bakers.get(&self.baker).map_or(false, |baker| {
            matches!(
                baker.block_endorser,
                BakerBlockEndorserState::RightsGetSuccess { .. }
            )
        })
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerBlockEndorserPreendorsementSignPendingAction {
    pub baker: SignaturePublicKey,
    pub operation: PreendorsementWithForgedBytes,
}

impl EnablingCondition<State> for BakerBlockEndorserPreendorsementSignPendingAction {
    fn is_enabled(&self, state: &State) -> bool {
        let baker_state = match state.bakers.get(&self.baker) {
            Some(v) => v,
            None => return false,
        };
        let preendorsement = self.operation.operation();
        match &baker_state.block_endorser {
            BakerBlockEndorserState::Preendorse { first_slot, .. } => {
                preendorsement.slot == *first_slot
                    && state
                        .current_head
                        .level()
                        .map_or(false, |level| preendorsement.level == level)
                    && state
                        .current_head
                        .round()
                        .map_or(false, |round| preendorsement.round == round)
                    && state
                        .current_head
                        .payload_hash()
                        .map_or(false, |p_hash| &preendorsement.block_payload_hash == p_hash)
            }
            _ => false,
        }
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerBlockEndorserPreendorsementSignSuccessAction {
    pub baker: SignaturePublicKey,
    pub signature: Signature,
}

impl EnablingCondition<State> for BakerBlockEndorserPreendorsementSignSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        state.bakers.get(&self.baker).map_or(false, |baker| {
            matches!(
                baker.block_endorser,
                BakerBlockEndorserState::PreendorsementSignPending { .. }
            )
        })
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerBlockEndorserPreendorsementInjectPendingAction {
    pub baker: SignaturePublicKey,
}

impl EnablingCondition<State> for BakerBlockEndorserPreendorsementInjectPendingAction {
    fn is_enabled(&self, state: &State) -> bool {
        state.bakers.get(&self.baker).map_or(false, |baker| {
            matches!(
                baker.block_endorser,
                BakerBlockEndorserState::PreendorsementSignSuccess { .. }
            )
        })
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerBlockEndorserPreendorsementInjectSuccessAction {
    pub baker: SignaturePublicKey,
}

impl EnablingCondition<State> for BakerBlockEndorserPreendorsementInjectSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        state.bakers.get(&self.baker).map_or(false, |baker| {
            matches!(
                baker.block_endorser,
                BakerBlockEndorserState::PreendorsementInjectPending { .. }
            )
        })
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerBlockEndorserPrequorumPendingAction {
    pub baker: SignaturePublicKey,
}

impl EnablingCondition<State> for BakerBlockEndorserPrequorumPendingAction {
    fn is_enabled(&self, state: &State) -> bool {
        state.bakers.get(&self.baker).map_or(false, |baker| {
            matches!(
                baker.block_endorser,
                BakerBlockEndorserState::PreendorsementInjectSuccess { .. }
            ) && !state.mempool.prequorum.is_reached()
        })
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerBlockEndorserPrequorumSuccessAction {
    pub baker: SignaturePublicKey,
}

impl EnablingCondition<State> for BakerBlockEndorserPrequorumSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        state.bakers.get(&self.baker).map_or(false, |baker| {
            matches!(
                baker.block_endorser,
                BakerBlockEndorserState::PreQuorumPending { .. }
            ) && state.mempool.prequorum.is_reached()
        })
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerBlockEndorserEndorseAction {
    pub baker: SignaturePublicKey,
}

impl EnablingCondition<State> for BakerBlockEndorserEndorseAction {
    fn is_enabled(&self, state: &State) -> bool {
        state.mempool.prequorum.is_reached()
            && state
                .bakers
                .get(&self.baker)
                .map_or(false, |baker| match &baker.block_endorser {
                    BakerBlockEndorserState::PreQuorumSuccess { .. } => true,
                    BakerBlockEndorserState::PreendorsementInjectSuccess { .. } => true,
                    _ => false,
                })
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerBlockEndorserEndorsementSignPendingAction {
    pub baker: SignaturePublicKey,
    pub operation: EndorsementWithForgedBytes,
}

impl EnablingCondition<State> for BakerBlockEndorserEndorsementSignPendingAction {
    fn is_enabled(&self, state: &State) -> bool {
        let baker_state = match state.bakers.get(&self.baker) {
            Some(v) => v,
            None => return false,
        };
        let endorsement = self.operation.operation();

        matches!(
            &baker_state.block_endorser,
            BakerBlockEndorserState::Endorse { .. }
        ) && baker_state
            .block_endorser
            .first_slot()
            .map_or(false, |slot| slot == endorsement.slot)
            && state
                .current_head
                .level()
                .map_or(false, |level| endorsement.level == level)
            && state
                .current_head
                .round()
                .map_or(false, |round| endorsement.round == round)
            && state
                .current_head
                .payload_hash()
                .map_or(false, |p_hash| &endorsement.block_payload_hash == p_hash)
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerBlockEndorserEndorsementSignSuccessAction {
    pub baker: SignaturePublicKey,
    pub signature: Signature,
}

impl EnablingCondition<State> for BakerBlockEndorserEndorsementSignSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        state.bakers.get(&self.baker).map_or(false, |baker| {
            matches!(
                baker.block_endorser,
                BakerBlockEndorserState::EndorsementSignPending { .. }
            )
        })
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerBlockEndorserEndorsementInjectPendingAction {
    pub baker: SignaturePublicKey,
}

impl EnablingCondition<State> for BakerBlockEndorserEndorsementInjectPendingAction {
    fn is_enabled(&self, state: &State) -> bool {
        state.bakers.get(&self.baker).map_or(false, |baker| {
            matches!(
                baker.block_endorser,
                BakerBlockEndorserState::EndorsementSignSuccess { .. }
            )
        })
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerBlockEndorserEndorsementInjectSuccessAction {
    pub baker: SignaturePublicKey,
}

impl EnablingCondition<State> for BakerBlockEndorserEndorsementInjectSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        state.bakers.get(&self.baker).map_or(false, |baker| {
            matches!(
                baker.block_endorser,
                BakerBlockEndorserState::EndorsementInjectPending { .. }
            )
        })
    }
}
