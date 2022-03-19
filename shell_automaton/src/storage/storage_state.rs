// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use crate::request::PendingRequests;

use super::blocks::StorageBlocksState;
use super::request::StorageRequestState;
use super::state_snapshot::StorageStateSnapshotState;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageState {
    pub requests: PendingRequests<StorageRequestState>,

    pub state_snapshot: StorageStateSnapshotState,

    pub blocks: StorageBlocksState,

    pub block_meta: super::kv_block_meta::State,
    pub block_additional_data: super::kv_block_additional_data::State,
    pub block_header: super::kv_block_header::State,
    pub operation: super::kv_operations::State,
    pub constants: super::kv_constants::State,
    pub cycle_data: super::kv_cycle_meta::State,
    pub cycle_eras: super::kv_cycle_eras::State,
}

impl StorageState {
    pub fn new() -> Self {
        Self {
            requests: PendingRequests::new(),

            state_snapshot: StorageStateSnapshotState::new(),

            blocks: StorageBlocksState::new(),

            block_meta: Default::default(),
            block_additional_data: Default::default(),
            block_header: Default::default(),
            operation: Default::default(),
            constants: Default::default(),
            cycle_data: Default::default(),
            cycle_eras: Default::default(),
        }
    }
}

impl Default for StorageState {
    fn default() -> Self {
        Self::new()
    }
}
