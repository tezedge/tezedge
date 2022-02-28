// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use tezos_api::ffi::InitProtocolContextResult;
use tezos_protocol_ipc_client::ProtocolServiceError;

use crate::protocol_runner::init::ProtocolRunnerInitState;
use crate::protocol_runner::{ProtocolRunnerState, ProtocolRunnerToken};
use crate::{EnablingCondition, State};

use super::ProtocolRunnerInitContextState;

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProtocolRunnerInitContextAction {}

impl EnablingCondition<State> for ProtocolRunnerInitContextAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.protocol_runner {
            ProtocolRunnerState::Init(ProtocolRunnerInitState::CheckGenesisAppliedSuccess {
                ..
            }) => true,
            _ => false,
        }
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProtocolRunnerInitContextPendingAction {
    pub token: ProtocolRunnerToken,
}

impl EnablingCondition<State> for ProtocolRunnerInitContextPendingAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.protocol_runner {
            ProtocolRunnerState::Init(ProtocolRunnerInitState::Context(
                ProtocolRunnerInitContextState::Init { .. },
            )) => true,
            _ => false,
        }
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProtocolRunnerInitContextErrorAction {
    pub token: Option<ProtocolRunnerToken>,
    pub error: ProtocolServiceError,
}

impl EnablingCondition<State> for ProtocolRunnerInitContextErrorAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.protocol_runner {
            ProtocolRunnerState::Init(ProtocolRunnerInitState::Context(state)) => match state {
                ProtocolRunnerInitContextState::Init { .. } => self.token.is_none(),
                ProtocolRunnerInitContextState::Pending { token, .. } => {
                    self.token.filter(|v| v == token).is_some()
                }
                _ => false,
            },
            _ => false,
        }
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProtocolRunnerInitContextSuccessAction {
    pub token: ProtocolRunnerToken,
    pub result: InitProtocolContextResult,
}

impl EnablingCondition<State> for ProtocolRunnerInitContextSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.protocol_runner {
            ProtocolRunnerState::Init(ProtocolRunnerInitState::Context(
                ProtocolRunnerInitContextState::Pending { token, .. },
            )) => self.token == *token,
            _ => false,
        }
    }
}
