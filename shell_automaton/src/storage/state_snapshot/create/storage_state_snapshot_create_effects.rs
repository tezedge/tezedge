use redux_rs::{ActionWithId, Store};

use crate::action::Action;
use crate::service::Service;
use crate::storage::request::StorageRequestInitAction;
use crate::State;

pub fn storage_state_snapshot_create_effects<S>(
    store: &mut Store<State, S, Action>,
    action: &ActionWithId<Action>,
) where
    S: Service,
{
    match &action.action {
        Action::StorageStateSnapshotCreate(_) => {
            store.dispatch(
                StorageRequestInitAction {
                    req_id: store.state().storage.requests.last_added_req_id(),
                }
                .into(),
            );
        }
        _ => {}
    }
}
