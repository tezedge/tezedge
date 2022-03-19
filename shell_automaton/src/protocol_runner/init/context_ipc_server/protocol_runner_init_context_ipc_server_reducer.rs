// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::protocol_runner::init::context::ProtocolRunnerInitContextState;
use crate::protocol_runner::init::ProtocolRunnerInitState;
use crate::protocol_runner::ProtocolRunnerState;
use crate::{Action, ActionWithMeta, State};

use super::ProtocolRunnerInitContextIpcServerState;

pub fn protocol_runner_init_context_ipc_server_reducer(state: &mut State, action: &ActionWithMeta) {
    match &action.action {
        Action::ProtocolRunnerInitContextIpcServer(_) => {
            let result = match &state.protocol_runner {
                ProtocolRunnerState::Init(ProtocolRunnerInitState::Context(
                    ProtocolRunnerInitContextState::Success { result, .. },
                )) => result,
                _ => return,
            }
            .clone();
            state.protocol_runner =
                ProtocolRunnerState::Init(ProtocolRunnerInitState::ContextIpcServer((
                    result,
                    ProtocolRunnerInitContextIpcServerState::Init {},
                )));
        }
        Action::ProtocolRunnerInitContextIpcServerPending(content) => {
            if let ProtocolRunnerState::Init(ProtocolRunnerInitState::ContextIpcServer((
                _,
                state,
            ))) = &mut state.protocol_runner
            {
                *state = ProtocolRunnerInitContextIpcServerState::Pending {
                    token: content.token,
                }
            }
        }
        Action::ProtocolRunnerInitContextIpcServerError(content) => {
            if let ProtocolRunnerState::Init(ProtocolRunnerInitState::ContextIpcServer((
                _,
                state,
            ))) = &mut state.protocol_runner
            {
                *state = ProtocolRunnerInitContextIpcServerState::Error {
                    token: content.token,
                    error: content.error.clone(),
                }
            }
        }
        Action::ProtocolRunnerInitContextIpcServerSuccess(content) => {
            if let ProtocolRunnerState::Init(ProtocolRunnerInitState::ContextIpcServer((
                _,
                state,
            ))) = &mut state.protocol_runner
            {
                *state = ProtocolRunnerInitContextIpcServerState::Success {
                    token: content.token,
                }
            }
        }
        _ => {}
    }
}
