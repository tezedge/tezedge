// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use tezos_protocol_ipc_client::ProtocolServiceError;

use crate::protocol_runner::init::ProtocolRunnerInitState;
use crate::protocol_runner::{ProtocolRunnerState, ProtocolRunnerToken};
use crate::{EnablingCondition, State};

use super::ProtocolRunnerInitRuntimeState;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProtocolRunnerInitRuntimeAction {}

impl EnablingCondition<State> for ProtocolRunnerInitRuntimeAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.protocol_runner {
            ProtocolRunnerState::Init(ProtocolRunnerInitState::Init { .. }) => true,
            _ => false,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProtocolRunnerInitRuntimePendingAction {
    pub token: ProtocolRunnerToken,
}

impl EnablingCondition<State> for ProtocolRunnerInitRuntimePendingAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.protocol_runner {
            ProtocolRunnerState::Init(ProtocolRunnerInitState::Runtime(
                ProtocolRunnerInitRuntimeState::Init,
            )) => true,
            _ => false,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProtocolRunnerInitRuntimeErrorAction {
    pub token: ProtocolRunnerToken,
    pub error: ProtocolServiceError,
}

impl EnablingCondition<State> for ProtocolRunnerInitRuntimeErrorAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.protocol_runner {
            ProtocolRunnerState::Init(ProtocolRunnerInitState::Runtime(
                ProtocolRunnerInitRuntimeState::Pending { token },
            )) => self.token == *token,
            _ => false,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProtocolRunnerInitRuntimeSuccessAction {
    pub token: ProtocolRunnerToken,
}

impl EnablingCondition<State> for ProtocolRunnerInitRuntimeSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.protocol_runner {
            ProtocolRunnerState::Init(ProtocolRunnerInitState::Runtime(
                ProtocolRunnerInitRuntimeState::Pending { token },
            )) => self.token == *token,
            _ => false,
        }
    }
}
