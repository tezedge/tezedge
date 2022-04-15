// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::{EnablingCondition, State};
use tezos_messages::p2p::encoding::block_header::BlockHeader;

use crate::{
    protocol_runner::ProtocolRunnerToken, rights::cycle_delegates::CycleDelegatesQueryState,
    storage::kv_cycle_meta::Cycle,
};

use super::{Delegates, DelegatesError};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
pub struct RightsCycleDelegatesGetAction {
    pub cycle: Cycle,
    pub block_header: BlockHeader,
}

impl EnablingCondition<State> for RightsCycleDelegatesGetAction {
    fn is_enabled(&self, state: &State) -> bool {
        !state.rights.cycle_delegates.contains_key(&self.cycle)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
pub struct RightsCycleDelegatesRequestedAction {
    pub cycle: Cycle,
    pub token: ProtocolRunnerToken,
}

impl EnablingCondition<State> for RightsCycleDelegatesRequestedAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .rights
            .cycle_delegates
            .get(&self.cycle)
            .map_or(false, |query| {
                matches!(query.state, CycleDelegatesQueryState::Init)
            })
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
pub struct RightsCycleDelegatesSuccessAction {
    pub cycle: Cycle,
    pub delegates: Delegates,
}

impl EnablingCondition<State> for RightsCycleDelegatesSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .rights
            .cycle_delegates
            .get(&self.cycle)
            .map_or(false, |query| {
                matches!(query.state, CycleDelegatesQueryState::ContextRequested(..))
            })
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
pub struct RightsCycleDelegatesErrorAction {
    pub cycle: Cycle,
    pub error: DelegatesError,
}

impl EnablingCondition<State> for RightsCycleDelegatesErrorAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .rights
            .cycle_delegates
            .get(&self.cycle)
            .map_or(false, |query| {
                matches!(query.state, CycleDelegatesQueryState::ContextRequested(..))
            })
    }
}
