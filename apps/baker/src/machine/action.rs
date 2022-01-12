// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::path::PathBuf;

use derive_more::From;
use serde::{Deserialize, Serialize};

use redux_rs::EnablingCondition;

use super::state::State;

#[derive(Serialize, Deserialize)]
pub struct RunWithLocalNodeAction {
    pub base_dir: PathBuf,
    pub node_dir: PathBuf,
    pub baker: String,
}

impl EnablingCondition<State> for RunWithLocalNodeAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}

#[derive(Serialize, Deserialize)]
pub struct BootstrappedAction;

impl EnablingCondition<State> for BootstrappedAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}

#[derive(Serialize, Deserialize)]
pub struct NewHeadSeenAction {
    pub head: serde_json::Value,
}

impl EnablingCondition<State> for NewHeadSeenAction {
    fn is_enabled(&self, state: &State) -> bool {
        state.is_bootstrapped
    }
}

#[derive(Serialize, Deserialize)]
pub struct NewOperationSeenAction {
    pub operation: serde_json::Value,
}

impl EnablingCondition<State> for NewOperationSeenAction {
    fn is_enabled(&self, state: &State) -> bool {
        state.is_bootstrapped
    }
}

#[derive(Serialize, Deserialize, From)]
#[serde(tag = "kind", content = "content")]
pub enum Action {
    RunWithLocalNode(RunWithLocalNodeAction),
    Bootstrapped(BootstrappedAction),
    NewHeadSeen(NewHeadSeenAction),
    NewOperationSeen(NewOperationSeenAction),
}
