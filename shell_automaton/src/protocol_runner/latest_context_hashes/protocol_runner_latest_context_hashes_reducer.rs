// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::protocol_runner::init::ProtocolRunnerInitState;
use crate::protocol_runner::ProtocolRunnerState;
use crate::{Action, ActionWithMeta, State};

use super::ProtocolRunnerLatestContextHashesState;

pub fn protocol_runner_latest_context_hashes_reducer(state: &mut State, action: &ActionWithMeta) {
    match &action.action {
        Action::ProtocolRunnerLatestContextHashesInit(_) => {
            let genesis_commit_hash = match &state.protocol_runner {
                ProtocolRunnerState::Init(ProtocolRunnerInitState::Success {
                    genesis_commit_hash,
                }) => genesis_commit_hash.clone(),
                _ => return,
            };

            state.protocol_runner = ProtocolRunnerLatestContextHashesState::Init {
                genesis_commit_hash,
            }
            .into();
        }
        Action::ProtocolRunnerLatestContextHashesPending(content) => {
            let genesis_commit_hash = match &state.protocol_runner {
                ProtocolRunnerState::LatestContextHashesGet(
                    ProtocolRunnerLatestContextHashesState::Init {
                        genesis_commit_hash,
                    },
                ) => genesis_commit_hash.clone(),
                _ => return,
            };

            state.protocol_runner = ProtocolRunnerLatestContextHashesState::Pending {
                token: content.token,
                genesis_commit_hash,
            }
            .into();
        }
        Action::ProtocolRunnerLatestContextHashesError(content) => {
            let genesis_commit_hash = match &state.protocol_runner {
                ProtocolRunnerState::LatestContextHashesGet(
                    ProtocolRunnerLatestContextHashesState::Pending {
                        genesis_commit_hash,
                        ..
                    },
                ) => genesis_commit_hash.clone(),
                _ => return,
            };

            state.protocol_runner = ProtocolRunnerLatestContextHashesState::Error {
                token: content.token,
                error: content.error.clone(),
                genesis_commit_hash,
                latest_context_hashes: Vec::new(),
            }
            .into();
        }
        Action::ProtocolRunnerLatestContextHashesSuccess(content) => {
            let genesis_commit_hash = match &state.protocol_runner {
                ProtocolRunnerState::LatestContextHashesGet(
                    ProtocolRunnerLatestContextHashesState::Pending {
                        genesis_commit_hash,
                        ..
                    },
                ) => genesis_commit_hash.clone(),
                _ => return,
            };

            state.protocol_runner = ProtocolRunnerLatestContextHashesState::Success {
                token: content.token,
                genesis_commit_hash,
                latest_context_hashes: content.latest_context_hashes.clone(),
            }
            .into();
        }
        _ => {}
    }
}
