// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use tezos_messages::base::signature_public_key::SignaturePublicKeyHash;

use crate::baker::persisted::PersistedState;
use crate::request::RequestId;
use crate::{EnablingCondition, State};

use super::BakerPersistedRehydrateState;

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerPersistedRehydrateInitAction {
    pub baker: SignaturePublicKeyHash,
}

impl EnablingCondition<State> for BakerPersistedRehydrateInitAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .bakers
            .get(&self.baker)
            .map_or(false, |baker| match &baker.persisted.rehydrate {
                BakerPersistedRehydrateState::Idle { .. } => true,
                _ => false,
            })
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerPersistedRehydratePendingAction {
    pub baker: SignaturePublicKeyHash,
    pub req_id: RequestId,
}

impl EnablingCondition<State> for BakerPersistedRehydratePendingAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .bakers
            .get(&self.baker)
            .map_or(false, |baker| match &baker.persisted.rehydrate {
                BakerPersistedRehydrateState::Idle { .. } => true,
                _ => false,
            })
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerPersistedRehydrateSuccessAction {
    pub baker: SignaturePublicKeyHash,
    pub result: PersistedState,
}

impl EnablingCondition<State> for BakerPersistedRehydrateSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .bakers
            .get(&self.baker)
            .map_or(false, |baker| match &baker.persisted.rehydrate {
                BakerPersistedRehydrateState::Pending { .. } => true,
                _ => false,
            })
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerPersistedRehydratedAction {
    pub baker: SignaturePublicKeyHash,
}

impl EnablingCondition<State> for BakerPersistedRehydratedAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .bakers
            .get(&self.baker)
            .map_or(false, |baker| match &baker.persisted.rehydrate {
                BakerPersistedRehydrateState::Success { .. } => true,
                _ => false,
            })
    }
}
