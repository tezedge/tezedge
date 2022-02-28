// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::{Action, ActionWithMeta, State};

use super::StorageBlocksGenesisInitState;

pub fn storage_blocks_genesis_init_reducer(state: &mut State, action: &ActionWithMeta) {
    match &action.action {
        Action::StorageBlocksGenesisInitSuccess(_) => {
            state.storage.blocks.genesis.init = StorageBlocksGenesisInitState::Success.into();
        }
        _ => {}
    }
}
