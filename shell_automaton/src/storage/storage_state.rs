// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use crate::request::PendingRequests;

use super::request::StorageRequestState;
use super::state_snapshot::StorageStateSnapshotState;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageState {
    pub requests: PendingRequests<StorageRequestState>,

    pub state_snapshot: StorageStateSnapshotState,
}

impl StorageState {
    pub fn new() -> Self {
        Self {
            requests: PendingRequests::new(),

            state_snapshot: StorageStateSnapshotState::new(),
        }
    }
}
