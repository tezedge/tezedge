// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::{Action, ActionWithMeta, State};

use super::StorageBlocksGenesisInitState;

pub fn storage_blocks_genesis_init_reducer(state: &mut State, action: &ActionWithMeta) {
    if let Action::StorageBlocksGenesisInitSuccess(_) = &action.action {
        state.storage.blocks.genesis.init = StorageBlocksGenesisInitState::Success;
    }
}
