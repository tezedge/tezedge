// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use crate::service::storage_service::StorageError;
use crate::{ActionId, EnablingCondition, State};

use super::StorageStateSnapshotCreateState;

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageStateSnapshotCreateInitAction {}

impl EnablingCondition<State> for StorageStateSnapshotCreateInitAction {
    fn is_enabled(&self, state: &State) -> bool {
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

impl EnablingCondition<State> for StorageStateSnapshotCreatePendingAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageStateSnapshotCreateErrorAction {
    pub action_id: ActionId,
    pub error: StorageError,
}

impl EnablingCondition<State> for StorageStateSnapshotCreateErrorAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageStateSnapshotCreateSuccessAction {
    pub action_id: ActionId,
}

impl EnablingCondition<State> for StorageStateSnapshotCreateSuccessAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}
