// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::{Action, ActionWithMeta, State};

use super::init::ProtocolRunnerInitState;
use super::{ProtocolRunnerReadyState, ProtocolRunnerState};

pub fn protocol_runner_reducer(state: &mut State, action: &ActionWithMeta) {
    match &action.action {
        Action::ProtocolRunnerReady(_) => {
            let genesis_commit_hash = match &state.protocol_runner {
                ProtocolRunnerState::Init(ProtocolRunnerInitState::Success {
                    genesis_commit_hash,
                }) => genesis_commit_hash.clone(),
                _ => return,
            };

            state.protocol_runner = ProtocolRunnerReadyState {
                genesis_commit_hash,
            }
            .into();
        }
        Action::ProtocolRunnerShutdownPending(_) => {
            state.protocol_runner = ProtocolRunnerState::ShutdownPending;
        }
        Action::ProtocolRunnerShutdownSuccess(_) => {
            state.protocol_runner = ProtocolRunnerState::ShutdownSuccess;
        }
        _ => {}
    }
}
