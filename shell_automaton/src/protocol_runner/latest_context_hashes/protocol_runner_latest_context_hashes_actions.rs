// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crypto::hash::ContextHash;
use serde::{Deserialize, Serialize};

use tezos_protocol_ipc_client::ProtocolServiceError;

use crate::protocol_runner::init::ProtocolRunnerInitState;
use crate::protocol_runner::{ProtocolRunnerState, ProtocolRunnerToken};
use crate::{EnablingCondition, State};

use super::ProtocolRunnerLatestContextHashesState;

pub const DEFAULT_NUMBER_OF_CONTEXT_HASHES: i64 = 10;

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProtocolRunnerLatestContextHashesInitAction {}

impl EnablingCondition<State> for ProtocolRunnerLatestContextHashesInitAction {
    fn is_enabled(&self, state: &State) -> bool {
        matches!(
            &state.protocol_runner,
            ProtocolRunnerState::Init(ProtocolRunnerInitState::Success { .. })
        )
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProtocolRunnerLatestContextHashesPendingAction {
    pub token: ProtocolRunnerToken,
}

impl EnablingCondition<State> for ProtocolRunnerLatestContextHashesPendingAction {
    fn is_enabled(&self, state: &State) -> bool {
        matches!(
            &state.protocol_runner,
            ProtocolRunnerState::LatestContextHashesGet(
                ProtocolRunnerLatestContextHashesState::Init { .. }
            )
        )
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProtocolRunnerLatestContextHashesErrorAction {
    pub token: ProtocolRunnerToken,
    pub error: ProtocolServiceError,
}

impl EnablingCondition<State> for ProtocolRunnerLatestContextHashesErrorAction {
    fn is_enabled(&self, state: &State) -> bool {
        matches!(
            &state.protocol_runner,
            ProtocolRunnerState::LatestContextHashesGet(
                ProtocolRunnerLatestContextHashesState::Pending { .. }
            )
        )
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProtocolRunnerLatestContextHashesSuccessAction {
    pub token: ProtocolRunnerToken,
    pub latest_context_hashes: Vec<ContextHash>,
}

impl EnablingCondition<State> for ProtocolRunnerLatestContextHashesSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        matches!(
            &state.protocol_runner,
            ProtocolRunnerState::LatestContextHashesGet(
                ProtocolRunnerLatestContextHashesState::Pending { .. }
            )
        )
    }
}
