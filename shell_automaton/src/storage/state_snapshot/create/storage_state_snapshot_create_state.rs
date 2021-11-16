// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use crate::service::storage_service::StorageError;
use crate::ActionId;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum StorageStateSnapshotCreateState {
    Idle,
    Pending {
        action_id: ActionId,
        /// applied_actions_count in the snapshot.
        applied_actions_count: u64,
    },
    Error {
        action_id: ActionId,
        /// applied_actions_count in the snapshot.
        applied_actions_count: u64,
        error: StorageError,
    },
    Success {
        action_id: ActionId,
        /// applied_actions_count in the snapshot.
        applied_actions_count: u64,
    },
}

impl StorageStateSnapshotCreateState {
    pub fn is_pending(&self) -> bool {
        matches!(self, Self::Pending { .. })
    }
}
