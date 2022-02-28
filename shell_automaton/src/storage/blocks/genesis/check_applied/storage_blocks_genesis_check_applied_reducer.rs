// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::{Action, ActionWithMeta, State};

use super::StorageBlocksGenesisCheckAppliedState;

pub fn storage_blocks_genesis_check_applied_reducer(state: &mut State, action: &ActionWithMeta) {
    match &action.action {
        Action::StorageBlocksGenesisCheckAppliedInit(_) => {
            state.storage.blocks.genesis.check_applied =
                StorageBlocksGenesisCheckAppliedState::GetMetaInit;
        }
        Action::StorageBlocksGenesisCheckAppliedGetMetaPending(_) => {
            let req_id = state.storage.requests.last_added_req_id();
            state.storage.blocks.genesis.check_applied =
                StorageBlocksGenesisCheckAppliedState::GetMetaPending { req_id };
        }
        Action::StorageBlocksGenesisCheckAppliedGetMetaError(_) => {
            state.storage.blocks.genesis.check_applied =
                StorageBlocksGenesisCheckAppliedState::GetMetaError {};
        }
        Action::StorageBlocksGenesisCheckAppliedGetMetaSuccess(content) => {
            state.storage.blocks.genesis.check_applied =
                StorageBlocksGenesisCheckAppliedState::GetMetaSuccess {
                    meta: content.meta.clone(),
                };
        }
        Action::StorageBlocksGenesisCheckAppliedSuccess(content) => {
            state.storage.blocks.genesis.check_applied =
                StorageBlocksGenesisCheckAppliedState::Success {
                    is_applied: content.is_applied,
                };
        }
        _ => {}
    }
}
