// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::{Action, ActionWithMeta, State};

use super::ShutdownState;

pub fn shutdown_reducer(state: &mut State, action: &ActionWithMeta) {
    match &action.action {
        Action::ShutdownInit(_) => {
            state.shutdown = ShutdownState::Init {
                time: action.time_as_nanos(),
            };
        }
        Action::ShutdownPending(_) => {
            state.shutdown = ShutdownState::pending(action.time_as_nanos());
        }
        Action::ShutdownSuccess(_) => {
            state.shutdown = ShutdownState::Success {
                time: action.time_as_nanos(),
            };
        }
        Action::ProtocolRunnerShutdownSuccess(_) => match &mut state.shutdown {
            ShutdownState::Pending(state) => state.protocol_runner_shutdown = true,
            _ => return,
        },
        _ => {}
    }
}
