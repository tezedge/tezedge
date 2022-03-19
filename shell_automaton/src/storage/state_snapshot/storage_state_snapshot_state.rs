// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use super::create::StorageStateSnapshotCreateState;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageStateSnapshotState {
    pub create: StorageStateSnapshotCreateState,
}

impl StorageStateSnapshotState {
    pub fn new() -> Self {
        Self {
            create: StorageStateSnapshotCreateState::Idle,
        }
    }
}

impl Default for StorageStateSnapshotState {
    fn default() -> Self {
        Self::new()
    }
}
