// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use redux_rs::EnablingCondition;

use crate::services::event::Block;

use super::BakerState;

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
pub struct IdleEventAction {}

impl<S> EnablingCondition<S> for IdleEventAction
where
    S: AsRef<BakerState>,
{
    fn is_enabled(&self, state: &S) -> bool {
        let _ = state;
        true
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
pub struct ProposalEventAction {
    pub block: Block,
}

impl<S> EnablingCondition<S> for ProposalEventAction
where
    S: AsRef<BakerState>,
{
    fn is_enabled(&self, state: &S) -> bool {
        let _ = state;
        true
    }
}

pub enum BakerAction {
    IdleEvent(IdleEventAction),
    ProposalEvent(ProposalEventAction),
}
