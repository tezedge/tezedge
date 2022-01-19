// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::protocol_runner::init::ProtocolRunnerInitState;
use crate::protocol_runner::ProtocolRunnerState;
use crate::{Action, ActionWithMeta, State};

use super::ProtocolRunnerInitContextState;

pub fn protocol_runner_init_context_reducer(state: &mut State, action: &ActionWithMeta) {
    match &action.action {
        Action::ProtocolRunnerInitContext(_) => {
            let apply_genesis = match &state.protocol_runner {
                ProtocolRunnerState::Init(
                    ProtocolRunnerInitState::CheckGenesisAppliedSuccess { is_applied },
                ) => !is_applied || state.config.init_storage_data.replay.is_some(),
                _ => return,
            };
            state.protocol_runner = ProtocolRunnerState::Init(
                ProtocolRunnerInitContextState::Init { apply_genesis }.into(),
            );
        }
        Action::ProtocolRunnerInitContextPending(content) => {
            let init_context_state = match &mut state.protocol_runner {
                ProtocolRunnerState::Init(ProtocolRunnerInitState::Context(v)) => v,
                _ => return,
            };
            let apply_genesis = init_context_state.apply_genesis();
            *init_context_state = ProtocolRunnerInitContextState::Pending {
                token: content.token,
                apply_genesis,
            };
        }
        Action::ProtocolRunnerInitContextError(content) => {
            let init_context_state = match &mut state.protocol_runner {
                ProtocolRunnerState::Init(ProtocolRunnerInitState::Context(v)) => v,
                _ => return,
            };
            let apply_genesis = init_context_state.apply_genesis();
            *init_context_state = ProtocolRunnerInitContextState::Error {
                token: content.token,
                apply_genesis,
            };
        }
        Action::ProtocolRunnerInitContextSuccess(content) => {
            let init_context_state = match &mut state.protocol_runner {
                ProtocolRunnerState::Init(ProtocolRunnerInitState::Context(v)) => v,
                _ => return,
            };
            let apply_genesis = init_context_state.apply_genesis();
            *init_context_state = ProtocolRunnerInitContextState::Success {
                token: content.token,
                apply_genesis,
                result: content.result.clone(),
            };
        }
        _ => {}
    }
}
