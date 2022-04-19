// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::protocol_runner::current_head::ProtocolRunnerCurrentHeadState;
use crate::{Action, ActionWithMeta, State};

use super::{ProtocolRunnerReadyState, ProtocolRunnerState};

pub fn protocol_runner_reducer(state: &mut State, action: &ActionWithMeta) {
    match &action.action {
        Action::ProtocolRunnerReady(_) => {
            let (genesis_commit_hash, latest_context_hashes) = match &state.protocol_runner {
                ProtocolRunnerState::GetCurrentHead(ProtocolRunnerCurrentHeadState::Success {
                    genesis_commit_hash,
                    latest_context_hashes,
                    ..
                }) => (genesis_commit_hash.clone(), latest_context_hashes.clone()),
                _ => return,
            };

            state.protocol_runner = ProtocolRunnerReadyState {
                genesis_commit_hash,
                latest_context_hashes,
            }
            .into();
        }
        Action::ProtocolRunnerShutdownPending(_) => {
            state.protocol_runner = ProtocolRunnerState::ShutdownPending;
        }
        Action::ProtocolRunnerShutdownSuccess(_) => {
            state.protocol_runner = ProtocolRunnerState::ShutdownSuccess;
        }
        _ => return,
    };
}
