// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::protocol_runner::ProtocolRunnerState;
use crate::{Action, ActionWithMeta, State};

use super::ProtocolRunnerInitState;

pub fn protocol_runner_init_reducer(state: &mut State, action: &ActionWithMeta) {
    match &action.action {
        Action::ProtocolRunnerInit(_) => {
            state.protocol_runner = ProtocolRunnerInitState::Init {}.into();
        }
        Action::ProtocolRunnerInitCheckGenesisApplied(_) => {
            state.protocol_runner = ProtocolRunnerInitState::CheckGenesisApplied {}.into();
        }
        Action::ProtocolRunnerInitCheckGenesisAppliedSuccess(content) => {
            state.protocol_runner = ProtocolRunnerInitState::CheckGenesisAppliedSuccess {
                is_applied: content.is_applied,
            }
            .into();
        }
        Action::ProtocolRunnerInitSuccess(_) => {
            let genesis_commit_hash = match &state.protocol_runner {
                ProtocolRunnerState::Init(ProtocolRunnerInitState::ContextIpcServer((
                    result,
                    _,
                ))) => result.genesis_commit_hash.clone(),
                _ => return,
            };
            state.protocol_runner = ProtocolRunnerInitState::Success {
                genesis_commit_hash,
            }
            .into();
        }
        _ => {}
    }
}
