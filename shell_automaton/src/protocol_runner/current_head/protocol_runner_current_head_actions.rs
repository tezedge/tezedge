// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crypto::hash::ContextHash;
use serde::{Deserialize, Serialize};

use tezos_protocol_ipc_client::ProtocolRunnerError;

use crate::protocol_runner::init::ProtocolRunnerInitState;
use crate::protocol_runner::{ProtocolRunnerState, ProtocolRunnerToken};
use crate::{EnablingCondition, State};

use super::ProtocolRunnerCurrentHeadState;

pub const DEFAULT_NUMBER_OF_CONTEXT_HASHES: i64 = 10;

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProtocolRunnerCurrentHeadInitAction {}

impl EnablingCondition<State> for ProtocolRunnerCurrentHeadInitAction {
    fn is_enabled(&self, state: &State) -> bool {
        matches!(
            &state.protocol_runner,
            ProtocolRunnerState::Init(ProtocolRunnerInitState::Success { .. })
        )
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProtocolRunnerCurrentHeadPendingAction {
    pub token: ProtocolRunnerToken,
}

impl EnablingCondition<State> for ProtocolRunnerCurrentHeadPendingAction {
    fn is_enabled(&self, state: &State) -> bool {
        matches!(
            &state.protocol_runner,
            ProtocolRunnerState::GetCurrentHead(ProtocolRunnerCurrentHeadState::Init { .. })
        )
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProtocolRunnerCurrentHeadErrorAction {
    pub token: ProtocolRunnerToken,
    pub error: ProtocolRunnerError,
}

impl EnablingCondition<State> for ProtocolRunnerCurrentHeadErrorAction {
    fn is_enabled(&self, state: &State) -> bool {
        matches!(
            &state.protocol_runner,
            ProtocolRunnerState::GetCurrentHead(ProtocolRunnerCurrentHeadState::Pending { .. })
        )
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProtocolRunnerCurrentHeadSuccessAction {
    pub token: ProtocolRunnerToken,
    pub latest_context_hashes: Vec<ContextHash>,
}

impl EnablingCondition<State> for ProtocolRunnerCurrentHeadSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        matches!(
            &state.protocol_runner,
            ProtocolRunnerState::GetCurrentHead(ProtocolRunnerCurrentHeadState::Pending { .. })
        )
    }
}
