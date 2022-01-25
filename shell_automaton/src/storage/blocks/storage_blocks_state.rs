// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use super::genesis::StorageBlocksGenesisState;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageBlocksState {
    pub genesis: StorageBlocksGenesisState,
}

impl StorageBlocksState {
    pub fn new() -> Self {
        Self {
            genesis: StorageBlocksGenesisState::new(),
        }
    }
}
