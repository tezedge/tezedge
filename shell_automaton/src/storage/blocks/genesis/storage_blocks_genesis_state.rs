// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use super::check_applied::StorageBlocksGenesisCheckAppliedState;
use super::init::StorageBlocksGenesisInitState;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageBlocksGenesisState {
    pub check_applied: StorageBlocksGenesisCheckAppliedState,
    pub init: StorageBlocksGenesisInitState,
}

impl StorageBlocksGenesisState {
    pub fn new() -> Self {
        Self {
            check_applied: StorageBlocksGenesisCheckAppliedState::Idle,
            init: StorageBlocksGenesisInitState::Idle,
        }
    }
}

impl Default for StorageBlocksGenesisState {
    fn default() -> Self {
        Self::new()
    }
}
