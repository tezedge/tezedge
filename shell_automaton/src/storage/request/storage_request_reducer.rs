use redux_rs::ActionWithId;

use crate::{action::Action, State};

use super::{StorageRequestState, StorageRequestStatus};

pub fn storage_request_reducer(state: &mut State, action: &ActionWithId<Action>) {
    match &action.action {
        Action::StorageRequestCreate(action) => {
            state.storage.requests.add(StorageRequestState {
                status: StorageRequestStatus::Idle,
                payload: action.payload.clone(),
            });
        }
        Action::StorageRequestPending(action) => {
            if let Some(req) = state.storage.requests.get_mut(action.req_id) {
                match &req.status {
                    StorageRequestStatus::Idle => {
                        req.status = StorageRequestStatus::Pending;
                    }
                    _ => return,
                }
            }
        }
        Action::StorageRequestError(action) => {
            if let Some(req) = state.storage.requests.get_mut(action.req_id) {
                match &req.status {
                    StorageRequestStatus::Idle | StorageRequestStatus::Pending => {
                        req.status = StorageRequestStatus::Error(action.error.clone());
                    }
                    _ => return,
                }
            }
        }
        Action::StorageRequestSuccess(action) => {
            if let Some(req) = state.storage.requests.get_mut(action.req_id) {
                match &req.status {
                    StorageRequestStatus::Pending => {
                        req.status = StorageRequestStatus::Success(action.result.clone());
                    }
                    _ => return,
                }
            }
        }
        Action::StorageRequestFinish(action) => {
            if let Some(req) = state.storage.requests.get(action.req_id) {
                match &req.status {
                    StorageRequestStatus::Error(_) | StorageRequestStatus::Success(_) => {
                        state.storage.requests.remove(action.req_id);
                    }
                    _ => return,
                }
            }
        }
        _ => {}
    }
}
