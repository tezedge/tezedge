// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::{Action, ActionWithMeta, State};

use super::StorageBlocksGenesisInitCommitResultGetState;

pub fn storage_blocks_genesis_init_commit_result_get_reducer(
    state: &mut State,
    action: &ActionWithMeta,
) {
    match &action.action {
        Action::StorageBlocksGenesisInitCommitResultGetInit(_) => {
            state.storage.blocks.genesis.init =
                StorageBlocksGenesisInitCommitResultGetState::Init {}.into();
        }
        Action::StorageBlocksGenesisInitCommitResultGetPending(content) => {
            state.storage.blocks.genesis.init =
                StorageBlocksGenesisInitCommitResultGetState::Pending {
                    token: content.token,
                }
                .into();
        }
        Action::StorageBlocksGenesisInitCommitResultGetError(content) => {
            state.storage.blocks.genesis.init =
                StorageBlocksGenesisInitCommitResultGetState::Error {
                    error: content.error.clone(),
                }
                .into();
        }
        Action::StorageBlocksGenesisInitCommitResultGetSuccess(content) => {
            state.storage.blocks.genesis.init =
                StorageBlocksGenesisInitCommitResultGetState::Success {
                    result: content.commit_result.clone(),
                }
                .into();
        }
        _ => {}
    }
}
