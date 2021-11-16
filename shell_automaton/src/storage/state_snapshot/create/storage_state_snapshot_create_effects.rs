// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use redux_rs::{ActionWithId, Store};

use crate::action::Action;
use crate::service::storage_service::{StorageRequest, StorageRequestPayload};
use crate::service::Service;
use crate::{State, StorageService};

use super::{StorageStateSnapshotCreateInitAction, StorageStateSnapshotCreatePendingAction};

pub fn storage_state_snapshot_create_effects<S>(
    store: &mut Store<State, S, Action>,
    action: &ActionWithId<Action>,
) where
    S: Service,
{
    match &action.action {
        Action::StorageStateSnapshotCreateInit(_) => {
            let req_payload =
                StorageRequestPayload::StateSnapshotPut(Box::new(store.state().clone()));
            let req = StorageRequest::new(None, req_payload).subscribe();
            let _ = store.service.storage().request_send(req);

            let action = StorageStateSnapshotCreatePendingAction {
                action_id: store.state().last_action.id(),
                applied_actions_count: store.state().applied_actions_count,
            };
            store.dispatch(action.into());
        }
        Action::StorageStateSnapshotCreateError(_)
        | Action::StorageStateSnapshotCreateSuccess(_) => {
            if StorageStateSnapshotCreateInitAction::enabling_condition(store.state()) {
                store.dispatch(StorageStateSnapshotCreateInitAction {}.into());
            }
        }
        _ => {}
    }
}
