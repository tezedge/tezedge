// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::protocol_runner::init::ProtocolRunnerInitState;
use crate::protocol_runner::ProtocolRunnerState;
use crate::{Action, ActionWithMeta, State};

use super::ProtocolRunnerCurrentHeadState;

pub fn protocol_runner_current_head_reducer(state: &mut State, action: &ActionWithMeta) {
    match &action.action {
        Action::ProtocolRunnerCurrentHeadInit(_) => {
            let genesis_commit_hash = match &state.protocol_runner {
                ProtocolRunnerState::Init(ProtocolRunnerInitState::Success {
                    genesis_commit_hash,
                }) => genesis_commit_hash.clone(),
                _ => return,
            };

            state.protocol_runner = ProtocolRunnerCurrentHeadState::Init {
                genesis_commit_hash,
            }
            .into();
        }
        Action::ProtocolRunnerCurrentHeadPending(content) => {
            let genesis_commit_hash = match &state.protocol_runner {
                ProtocolRunnerState::GetCurrentHead(ProtocolRunnerCurrentHeadState::Init {
                    genesis_commit_hash,
                }) => genesis_commit_hash.clone(),
                _ => return,
            };

            state.protocol_runner = ProtocolRunnerCurrentHeadState::Pending {
                token: content.token,
                genesis_commit_hash,
            }
            .into();
        }
        Action::ProtocolRunnerCurrentHeadError(content) => {
            let genesis_commit_hash = match &state.protocol_runner {
                ProtocolRunnerState::GetCurrentHead(ProtocolRunnerCurrentHeadState::Pending {
                    genesis_commit_hash,
                    ..
                }) => genesis_commit_hash.clone(),
                _ => return,
            };

            state.protocol_runner = ProtocolRunnerCurrentHeadState::Error {
                token: content.token,
                error: content.error.clone(),
                genesis_commit_hash,
            }
            .into();
        }
        Action::ProtocolRunnerCurrentHeadSuccess(content) => {
            let genesis_commit_hash = match &state.protocol_runner {
                ProtocolRunnerState::GetCurrentHead(ProtocolRunnerCurrentHeadState::Pending {
                    genesis_commit_hash,
                    ..
                }) => genesis_commit_hash.clone(),
                _ => return,
            };

            state.protocol_runner = ProtocolRunnerCurrentHeadState::Success {
                token: content.token,
                genesis_commit_hash,
                latest_context_hashes: content.latest_context_hashes.clone(),
            }
            .into();
        }
        _ => {}
    }
}
