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
            match &mut state.protocol_runner {
                ProtocolRunnerState::Init(ProtocolRunnerInitState::ContextIpcServer((
                    _,
                    state,
                ))) => {
                    *state = ProtocolRunnerInitContextIpcServerState::Pending {
                        token: content.token,
                    }
                }
                _ => return,
            }
        }
        Action::ProtocolRunnerInitContextIpcServerError(content) => {
            match &mut state.protocol_runner {
                ProtocolRunnerState::Init(ProtocolRunnerInitState::ContextIpcServer((
                    _,
                    state,
                ))) => {
                    *state = ProtocolRunnerInitContextIpcServerState::Error {
                        token: content.token,
                        error: content.error.clone(),
                    }
                }
                _ => return,
            }
        }
        Action::ProtocolRunnerInitContextIpcServerSuccess(content) => {
            match &mut state.protocol_runner {
                ProtocolRunnerState::Init(ProtocolRunnerInitState::ContextIpcServer((
                    _,
                    state,
                ))) => {
                    *state = ProtocolRunnerInitContextIpcServerState::Success {
                        token: content.token,
                    }
                }
                _ => return,
            }
        }
        _ => {}
    }
}
