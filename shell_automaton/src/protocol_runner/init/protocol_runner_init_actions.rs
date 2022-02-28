// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use crate::protocol_runner::spawn_server::ProtocolRunnerSpawnServerState;
use crate::protocol_runner::ProtocolRunnerState;
use crate::{EnablingCondition, State};

use super::context_ipc_server::ProtocolRunnerInitContextIpcServerState;
use super::runtime::ProtocolRunnerInitRuntimeState;
use super::ProtocolRunnerInitState;

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProtocolRunnerInitAction {}

impl EnablingCondition<State> for ProtocolRunnerInitAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.protocol_runner {
            ProtocolRunnerState::SpawnServer(ProtocolRunnerSpawnServerState::Success {
                ..
            }) => true,
            _ => false,
        }
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProtocolRunnerInitCheckGenesisAppliedAction {}

impl EnablingCondition<State> for ProtocolRunnerInitCheckGenesisAppliedAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.protocol_runner {
            ProtocolRunnerState::Init(ProtocolRunnerInitState::Runtime(
                ProtocolRunnerInitRuntimeState::Success { .. },
            )) => true,
            _ => false,
        }
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProtocolRunnerInitCheckGenesisAppliedSuccessAction {
    pub is_applied: bool,
}

impl EnablingCondition<State> for ProtocolRunnerInitCheckGenesisAppliedSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.protocol_runner {
            ProtocolRunnerState::Init(ProtocolRunnerInitState::CheckGenesisApplied { .. }) => true,
            _ => false,
        }
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProtocolRunnerInitSuccessAction {}

impl EnablingCondition<State> for ProtocolRunnerInitSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.protocol_runner {
            ProtocolRunnerState::Init(ProtocolRunnerInitState::ContextIpcServer((
                _,
                ProtocolRunnerInitContextIpcServerState::Success { .. },
            ))) => true,
            _ => false,
        }
    }
}
