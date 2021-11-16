// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use crate::service::storage_service::StorageError;
use crate::ActionId;
use crate::State;

use super::StorageStateSnapshotCreateState;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageStateSnapshotCreateInitAction {}

impl StorageStateSnapshotCreateInitAction {
    // TODO: move inside a trait that every action needs to implement and must
    // be checked on every dispatch call in redux_rs, before reducer is called,
    // to make sure that if action is not enabled, we don't dispatch it.
    pub fn enabling_condition(state: &State) -> bool {
        if let Some(interval) = state.config.record_state_snapshots_with_interval {
            match &state.storage.state_snapshot.create {
                // We haven't saved any snapshots so we should create one.
                StorageStateSnapshotCreateState::Idle => true,
                // Only one pending state snapshot at a time.
                StorageStateSnapshotCreateState::Pending { .. } => false,
                // We should repeat saving the snapshot if previous one failed.
                StorageStateSnapshotCreateState::Error { .. } => true,
                StorageStateSnapshotCreateState::Success {
                    applied_actions_count,
                    ..
                } => state.applied_actions_count - applied_actions_count >= interval,
            }
        } else {
            false
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageStateSnapshotCreatePendingAction {
    pub action_id: ActionId,
    pub applied_actions_count: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageStateSnapshotCreateErrorAction {
    pub action_id: ActionId,
    pub error: StorageError,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageStateSnapshotCreateSuccessAction {
    pub action_id: ActionId,
}
