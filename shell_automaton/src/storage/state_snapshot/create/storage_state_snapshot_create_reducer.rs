// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use redux_rs::ActionWithId;

use crate::action::Action;
use crate::State;

use super::StorageStateSnapshotCreateState;

pub fn storage_state_snapshot_create_reducer(state: &mut State, action: &ActionWithId<Action>) {
    match &action.action {
        Action::StorageStateSnapshotCreatePending(action) => {
            let state = &mut state.storage.state_snapshot.create;
            if !state.is_pending() {
                *state = StorageStateSnapshotCreateState::Pending {
                    action_id: action.action_id,
                    applied_actions_count: action.applied_actions_count,
                };
            }
        }
        Action::StorageStateSnapshotCreateError(action) => {
            let state = &mut state.storage.state_snapshot.create;
            let (action_id, applied_actions_count) = match state {
                StorageStateSnapshotCreateState::Pending {
                    action_id,
                    applied_actions_count,
                } => (*action_id, *applied_actions_count),
                _ => return,
            };

            if action_id == action.action_id {
                *state = StorageStateSnapshotCreateState::Error {
                    action_id: action.action_id,
                    applied_actions_count,
                    error: action.error.clone(),
                };
            }
        }
        Action::StorageStateSnapshotCreateSuccess(action) => {
            let state = &mut state.storage.state_snapshot.create;
            let (action_id, applied_actions_count) = match state {
                StorageStateSnapshotCreateState::Pending {
                    action_id,
                    applied_actions_count,
                } => (*action_id, *applied_actions_count),
                _ => return,
            };

            if action_id == action.action_id {
                *state = StorageStateSnapshotCreateState::Success {
                    action_id,
                    applied_actions_count,
                };
            }
        }
        _ => {}
    }
}
