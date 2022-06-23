// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use tezos_messages::base::signature_public_key::SignaturePublicKeyHash;

use crate::request::RequestId;
use crate::{EnablingCondition, State};

use super::BakerPersistedPersistState;

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerPersistedPersistInitAction {
    pub baker: SignaturePublicKeyHash,
}

impl EnablingCondition<State> for BakerPersistedPersistInitAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .bakers
            .get(&self.baker)
            .map_or(false, |baker| match &baker.persisted.persist {
                BakerPersistedPersistState::Idle { .. } => true,
                BakerPersistedPersistState::Success { counter, .. } => baker
                    .persisted
                    .current_state()
                    .map_or(false, |s| s.counter > *counter),
                _ => false,
            })
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerPersistedPersistPendingAction {
    pub baker: SignaturePublicKeyHash,
    pub req_id: RequestId,
    pub counter: u64,
}

impl EnablingCondition<State> for BakerPersistedPersistPendingAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .bakers
            .get(&self.baker)
            .map_or(false, |baker| match &baker.persisted.persist {
                BakerPersistedPersistState::Idle { .. } => true,
                BakerPersistedPersistState::Success { counter, .. } => baker
                    .persisted
                    .current_state()
                    .map_or(false, |s| s.counter == self.counter && s.counter > *counter),
                _ => false,
            })
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerPersistedPersistSuccessAction {
    pub baker: SignaturePublicKeyHash,
}

impl EnablingCondition<State> for BakerPersistedPersistSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .bakers
            .get(&self.baker)
            .map_or(false, |baker| match &baker.persisted.persist {
                BakerPersistedPersistState::Pending { .. } => true,
                _ => false,
            })
    }
}
