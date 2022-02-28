// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::storage::blocks::genesis::init::commit_result_get::StorageBlocksGenesisInitCommitResultGetState;
use crate::storage::blocks::genesis::init::StorageBlocksGenesisInitState;
use crate::{Action, ActionWithMeta, State};

use super::StorageBlocksGenesisInitCommitResultPutState;

pub fn storage_blocks_genesis_init_commit_result_put_reducer(
    state: &mut State,
    action: &ActionWithMeta,
) {
    match &action.action {
        Action::StorageBlocksGenesisInitCommitResultPutInit(_) => {
            let result = match &state.storage.blocks.genesis.init {
                StorageBlocksGenesisInitState::CommitResultGet(
                    StorageBlocksGenesisInitCommitResultGetState::Success { result },
                ) => result,
                _ => return,
            };
            state.storage.blocks.genesis.init =
                StorageBlocksGenesisInitCommitResultPutState::Init {
                    result: result.clone(),
                }
                .into();
        }
        Action::StorageBlocksGenesisInitCommitResultPutError(_) => {
            state.storage.blocks.genesis.init =
                StorageBlocksGenesisInitCommitResultPutState::Error {}.into();
        }
        Action::StorageBlocksGenesisInitCommitResultPutSuccess(_) => {
            state.storage.blocks.genesis.init =
                StorageBlocksGenesisInitCommitResultPutState::Success {}.into();
        }
        _ => {}
    }
}
