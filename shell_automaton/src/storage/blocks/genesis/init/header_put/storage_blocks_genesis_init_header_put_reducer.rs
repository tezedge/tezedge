// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::{Action, ActionWithMeta, State};

use super::StorageBlocksGenesisInitHeaderPutState;

pub fn storage_blocks_genesis_init_header_put_reducer(state: &mut State, action: &ActionWithMeta) {
    match &action.action {
        Action::StorageBlocksGenesisInitHeaderPutInit(_) => {
            state.storage.blocks.genesis.init = StorageBlocksGenesisInitHeaderPutState::Init.into();
        }
        Action::StorageBlocksGenesisInitHeaderPutPending(_) => {
            let req_id = state.storage.requests.last_added_req_id();
            state.storage.blocks.genesis.init =
                StorageBlocksGenesisInitHeaderPutState::Pending { req_id }.into();
        }
        Action::StorageBlocksGenesisInitHeaderPutError(_) => {
            state.storage.blocks.genesis.init =
                StorageBlocksGenesisInitHeaderPutState::Error {}.into();
        }
        Action::StorageBlocksGenesisInitHeaderPutSuccess(content) => {
            state.storage.blocks.genesis.init = StorageBlocksGenesisInitHeaderPutState::Success {
                is_new_block: content.is_new_block,
            }
            .into();
        }
        _ => {}
    }
}
