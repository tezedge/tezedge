// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::{Action, ActionWithMeta, State};

use super::ProtocolRunnerSpawnServerState;

pub fn protocol_runner_spawn_server_reducer(state: &mut State, action: &ActionWithMeta) {
    match &action.action {
        Action::ProtocolRunnerSpawnServerInit(_) => {
            state.protocol_runner = ProtocolRunnerSpawnServerState::Init.into();
        }
        Action::ProtocolRunnerSpawnServerPending(_) => {
            state.protocol_runner = ProtocolRunnerSpawnServerState::Pending {}.into();
        }
        Action::ProtocolRunnerSpawnServerError(content) => {
            state.protocol_runner = ProtocolRunnerSpawnServerState::Error {
                error: content.error.clone(),
            }
            .into();
        }
        Action::ProtocolRunnerSpawnServerSuccess(_) => {
            state.protocol_runner = ProtocolRunnerSpawnServerState::Success {}.into();
        }
        _ => {}
    }
}
