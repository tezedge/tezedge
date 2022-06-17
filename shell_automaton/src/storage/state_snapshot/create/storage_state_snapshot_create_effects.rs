// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::service::storage_service::{StorageRequest, StorageRequestPayload, StorageService};
use crate::{Action, ActionWithMeta, Service, Store};

use super::{StorageStateSnapshotCreateInitAction, StorageStateSnapshotCreatePendingAction};

pub fn storage_state_snapshot_create_effects<S>(store: &mut Store<S>, action: &ActionWithMeta)
where
    S: Service,
{
    match &action.action {
        Action::StorageStateSnapshotCreateInit(_) => {
            let req_payload =
                StorageRequestPayload::StateSnapshotPut(Box::new(store.state().clone()));
            let req = StorageRequest::new(None, req_payload).subscribe();
            let _ = store.service.storage().request_send(req).unwrap_or_else(|_| panic!("snapshot"));

            let action = StorageStateSnapshotCreatePendingAction {
                action_id: store.state().last_action.id(),
                applied_actions_count: store.state().applied_actions_count,
            };
            store.dispatch(action);
        }
        Action::StorageStateSnapshotCreateError(_)
        | Action::StorageStateSnapshotCreateSuccess(_) => {
            store.dispatch(StorageStateSnapshotCreateInitAction {});
        }
        _ => {}
    }
}
