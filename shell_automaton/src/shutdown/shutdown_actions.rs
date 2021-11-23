// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use crate::{EnablingCondition, State};

use super::ShutdownState;

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ShutdownInitAction {}

impl EnablingCondition<State> for ShutdownInitAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.shutdown {
            ShutdownState::Idle => true,
            _ => false,
        }
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ShutdownPendingAction {}

impl EnablingCondition<State> for ShutdownPendingAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.shutdown {
            ShutdownState::Init { .. } => true,
            _ => false,
        }
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ShutdownSuccessAction {}

impl EnablingCondition<State> for ShutdownSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        state.shutdown.is_pending_complete()
    }
}
