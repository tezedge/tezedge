// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::protocol_runner::ProtocolRunnerState;
use crate::{Action, ActionWithMeta, State};

use super::ProtocolRunnerInitRuntimeState;

pub fn protocol_runner_init_runtime_reducer(state: &mut State, action: &ActionWithMeta) {
    match &action.action {
        Action::ProtocolRunnerInitRuntime(_) => {
            state.protocol_runner =
                ProtocolRunnerState::Init(ProtocolRunnerInitRuntimeState::Init.into());
        }
        Action::ProtocolRunnerInitRuntimePending(content) => {
            state.protocol_runner = ProtocolRunnerState::Init(
                ProtocolRunnerInitRuntimeState::Pending {
                    token: content.token,
                }
                .into(),
            );
        }
        Action::ProtocolRunnerInitRuntimeError(content) => {
            state.protocol_runner = ProtocolRunnerState::Init(
                ProtocolRunnerInitRuntimeState::Error {
                    token: content.token,
                }
                .into(),
            );
        }
        Action::ProtocolRunnerInitRuntimeSuccess(content) => {
            state.protocol_runner = ProtocolRunnerState::Init(
                ProtocolRunnerInitRuntimeState::Success {
                    token: content.token,
                }
                .into(),
            );
        }
        _ => {}
    }
}
