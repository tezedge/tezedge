// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use crate::{EnablingCondition, State};

use super::{PausedLoop, PausedLoopCurrent};

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PausedLoopsAddAction {
    pub data: PausedLoop,
}

impl EnablingCondition<State> for PausedLoopsAddAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PausedLoopsResumeAllAction {}

impl EnablingCondition<State> for PausedLoopsResumeAllAction {
    fn is_enabled(&self, state: &State) -> bool {
        !state.paused_loops.is_empty()
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PausedLoopsResumeNextInitAction {}

impl EnablingCondition<State> for PausedLoopsResumeNextInitAction {
    fn is_enabled(&self, state: &State) -> bool {
        !state.paused_loops.is_empty()
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PausedLoopsResumeNextSuccessAction {}

impl EnablingCondition<State> for PausedLoopsResumeNextSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.paused_loops.current {
            PausedLoopCurrent::Init(_) => true,
            _ => false,
        }
    }
}
