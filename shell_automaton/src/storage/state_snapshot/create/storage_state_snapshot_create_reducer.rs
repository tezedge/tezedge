use redux_rs::ActionWithId;
use std::sync::Arc;

use crate::action::Action;
use crate::service::storage_service::StorageRequestPayload;
use crate::storage::request::{StorageRequestState, StorageRequestStatus};
use crate::State;

pub fn storage_state_snapshot_create_reducer(state: &mut State, action: &ActionWithId<Action>) {
    match &action.action {
        Action::StorageStateSnapshotCreate(_) => {
            state.storage.requests.add(StorageRequestState {
                status: StorageRequestStatus::Idle,
                payload: StorageRequestPayload::StateSnapshotPut(Arc::new(state.clone())),
            });
        }
        _ => {}
    }
}
