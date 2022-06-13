// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

pub mod block_baker;
pub mod block_endorser;
pub mod seed_nonce;

mod baker_state;
pub use baker_state::*;

mod baker_effects;
pub use baker_effects::*;

use tezos_messages::base::signature_public_key::SignaturePublicKeyHash;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
pub struct BakerAddAction {
    pub baker: SignaturePublicKeyHash,
}

impl crate::EnablingCondition<crate::State> for BakerAddAction {
    fn is_enabled(&self, state: &crate::State) -> bool {
        !state.bakers.contains_key(&self.baker)
    }
}

pub fn baker_reducer(state: &mut crate::State, action: &crate::ActionWithMeta) {
    match &action.action {
        crate::Action::BakerAdd(content) => {
            state.bakers.insert(
                content.baker.clone(),
                BakerState::new(state.config.liquidity_baking_escape_vote),
            );
        }
        _ => (),
    }
}
