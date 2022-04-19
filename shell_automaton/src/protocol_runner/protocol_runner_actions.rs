// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use crate::protocol_runner::current_head::ProtocolRunnerCurrentHeadState;
use crate::protocol_runner::ProtocolRunnerState;
use crate::service::protocol_runner_service::ProtocolRunnerResult;
use crate::storage::blocks::genesis::init::StorageBlocksGenesisInitState;
use crate::{EnablingCondition, State};

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProtocolRunnerStartAction {}

impl EnablingCondition<State> for ProtocolRunnerStartAction {
    fn is_enabled(&self, state: &State) -> bool {
        matches!(&state.protocol_runner, ProtocolRunnerState::Idle)
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProtocolRunnerReadyAction {}

impl EnablingCondition<State> for ProtocolRunnerReadyAction {
    fn is_enabled(&self, state: &State) -> bool {
        matches!(
            &state.protocol_runner,
            ProtocolRunnerState::GetCurrentHead(ProtocolRunnerCurrentHeadState::Success { .. })
        )
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProtocolRunnerResponseAction {
    pub result: ProtocolRunnerResult,
}

impl EnablingCondition<State> for ProtocolRunnerResponseAction {
    fn is_enabled(&self, state: &State) -> bool {
        matches!(&state.protocol_runner, ProtocolRunnerState::Ready(_))
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProtocolRunnerResponseUnexpectedAction {
    pub result: ProtocolRunnerResult,
}

impl EnablingCondition<State> for ProtocolRunnerResponseUnexpectedAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

/// Notify to pieces outside state machine about protocol runner's
/// or context's initialization status.
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProtocolRunnerNotifyStatusAction {}

impl EnablingCondition<State> for ProtocolRunnerNotifyStatusAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.protocol_runner {
            ProtocolRunnerState::Ready(s) => {
                s.genesis_commit_hash.is_none()
                    || matches!(
                        &state.storage.blocks.genesis.init,
                        StorageBlocksGenesisInitState::Success
                    )
            }
            _ => false,
        }
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProtocolRunnerShutdownInitAction {}

impl EnablingCondition<State> for ProtocolRunnerShutdownInitAction {
    fn is_enabled(&self, state: &State) -> bool {
        !matches!(
            &state.protocol_runner,
            ProtocolRunnerState::ShutdownPending | ProtocolRunnerState::ShutdownSuccess
        )
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProtocolRunnerShutdownPendingAction {}

impl EnablingCondition<State> for ProtocolRunnerShutdownPendingAction {
    fn is_enabled(&self, state: &State) -> bool {
        !matches!(
            &state.protocol_runner,
            ProtocolRunnerState::ShutdownPending | ProtocolRunnerState::ShutdownSuccess
        )
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProtocolRunnerShutdownSuccessAction {}

impl EnablingCondition<State> for ProtocolRunnerShutdownSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        matches!(&state.protocol_runner, ProtocolRunnerState::ShutdownPending)
    }
}
