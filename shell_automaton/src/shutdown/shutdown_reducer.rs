// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::{Action, ActionWithMeta, State};

use super::ShutdownState;

pub fn shutdown_reducer(state: &mut State, action: &ActionWithMeta) {
    match &action.action {
        Action::ShutdownInit(_) => {
            println!("***** shutdown init");
            state.shutdown = ShutdownState::Init {
                time: action.time_as_nanos(),
            };
        }
        Action::ShutdownPending(_) => {
            println!("***** shutdown pending");
            state.shutdown = ShutdownState::pending(action.time_as_nanos());
        }
        Action::ShutdownSuccess(_) => {
            println!("***** shutdown success");
            state.shutdown = ShutdownState::Success {
                time: action.time_as_nanos(),
            };
        }
        Action::ProtocolRunnerShutdownSuccess(_) => {
            println!("***** protocol shutdown success");
            if let ShutdownState::Pending(state) = &mut state.shutdown {
                state.protocol_runner_shutdown = true
            }
        }
        _ => {}
    }
}
