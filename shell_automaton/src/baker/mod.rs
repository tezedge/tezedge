// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

pub mod block_baker;
pub mod block_endorser;

mod baker_state;
pub use baker_state::*;

mod baker_effects;
pub use baker_effects::*;
use tezos_messages::base::signature_public_key::SignaturePublicKey;

// TODO(zura): read these constants from protocol.
pub const MINIMAL_BLOCK_DELAY: u64 = 15;
pub const DELAY_INCREMENT_PER_ROUND: u64 = 5;
pub const CONSENSUS_COMMITTEE_SIZE: u32 = 7000;
pub const BLOCKS_PER_COMMITMENT: i32 = 32;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
pub struct BakerAddAction {
    pub public_key: SignaturePublicKey,
}

impl crate::EnablingCondition<crate::State> for BakerAddAction {
    fn is_enabled(&self, state: &crate::State) -> bool {
        !state.bakers.contains_key(&self.public_key)
    }
}

pub fn baker_reducer(state: &mut crate::State, action: &crate::ActionWithMeta) {
    match &action.action {
        crate::Action::BakerAdd(content) => {
            state
                .bakers
                .insert(content.public_key.clone(), BakerState::new());
        }
        _ => (),
    }
}
