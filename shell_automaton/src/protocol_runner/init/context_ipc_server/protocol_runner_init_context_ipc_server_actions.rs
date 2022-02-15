// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use tezos_protocol_ipc_client::ProtocolServiceError;

use crate::protocol_runner::init::context::ProtocolRunnerInitContextState;
use crate::protocol_runner::init::ProtocolRunnerInitState;
use crate::protocol_runner::{ProtocolRunnerState, ProtocolRunnerToken};
use crate::{EnablingCondition, State};

use super::ProtocolRunnerInitContextIpcServerState;

/// Initializes server to listen for readonly context clients through IPC.
///
/// Must be called after the writable context has been initialized.
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProtocolRunnerInitContextIpcServerAction {}

impl EnablingCondition<State> for ProtocolRunnerInitContextIpcServerAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.protocol_runner {
            ProtocolRunnerState::Init(ProtocolRunnerInitState::Context(
                ProtocolRunnerInitContextState::Success { .. },
            )) => true,
            _ => false,
        }
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProtocolRunnerInitContextIpcServerPendingAction {
    pub token: ProtocolRunnerToken,
}

impl EnablingCondition<State> for ProtocolRunnerInitContextIpcServerPendingAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.protocol_runner {
            ProtocolRunnerState::Init(ProtocolRunnerInitState::ContextIpcServer((
                _,
                ProtocolRunnerInitContextIpcServerState::Init { .. },
            ))) => true,
            _ => false,
        }
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProtocolRunnerInitContextIpcServerErrorAction {
    pub token: ProtocolRunnerToken,
    pub error: ProtocolServiceError,
}

impl EnablingCondition<State> for ProtocolRunnerInitContextIpcServerErrorAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.protocol_runner {
            ProtocolRunnerState::Init(ProtocolRunnerInitState::ContextIpcServer((
                _,
                ProtocolRunnerInitContextIpcServerState::Pending { token, .. },
            ))) => self.token == *token,
            _ => false,
        }
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProtocolRunnerInitContextIpcServerSuccessAction {
    pub token: Option<ProtocolRunnerToken>,
}

impl EnablingCondition<State> for ProtocolRunnerInitContextIpcServerSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        let context_ipc_server_init_state = match &state.protocol_runner {
            ProtocolRunnerState::Init(ProtocolRunnerInitState::ContextIpcServer((_, v))) => v,
            _ => return false,
        };

        match context_ipc_server_init_state {
            ProtocolRunnerInitContextIpcServerState::Init {} => state
                .config
                .protocol_runner
                .storage
                .get_ipc_socket_path()
                .is_none(),
            ProtocolRunnerInitContextIpcServerState::Pending { token } => {
                self.token.filter(|v| v == token).is_some()
            }
            _ => false,
        }
    }
}
