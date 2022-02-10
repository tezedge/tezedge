// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::action::Action;
use crate::service::storage_service::StorageRequest;
use crate::service::storage_service::{StorageResponseError, StorageResponseSuccess};
use crate::service::{Service, StorageService};
use crate::storage::state_snapshot::create::{
    StorageStateSnapshotCreateErrorAction, StorageStateSnapshotCreateSuccessAction,
};
use crate::{ActionWithMeta, Store};

use super::{
    StorageRequestErrorAction, StorageRequestFinishAction, StorageRequestInitAction,
    StorageRequestPendingAction, StorageRequestStatus, StorageRequestSuccessAction,
    StorageRequestor, StorageResponseReceivedAction,
};

pub fn storage_request_effects<S>(store: &mut Store<S>, action: &ActionWithMeta)
where
    S: Service,
{
    match &action.action {
        Action::StorageRequestCreate(_) => {
            store.dispatch(StorageRequestInitAction {
                req_id: store.state().storage.requests.last_added_req_id(),
            });
        }
        Action::StorageRequestInit(action) => {
            let req = match store.state.get().storage.requests.get(action.req_id) {
                Some(v) => v,
                None => return,
            };
            match req.status {
                StorageRequestStatus::Idle => {}
                _ => return,
            }
            // TODO: handle send error in case of mpsc disconnection.
            store
                .service
                .storage()
                .request_send(StorageRequest::new(
                    Some(action.req_id),
                    req.payload.clone(),
                ))
                .unwrap();
            store.dispatch(StorageRequestPendingAction {
                req_id: action.req_id,
            });
        }
        Action::WakeupEvent(_) => {
            // TODO: handle disconnected error.
            while let Ok(response) = store.service.storage().response_try_recv() {
                let requestor = response
                    .req_id
                    .and_then(|req_id| store.state().storage.requests.get(req_id))
                    .map(|req| req.requestor.clone())
                    .unwrap_or(StorageRequestor::None);
                store.dispatch(StorageResponseReceivedAction {
                    response,
                    requestor,
                });
            }
        }
        Action::StorageResponseReceived(action) => {
            let resp = &action.response;
            if let Some(req_id) = resp.req_id {
                match resp.result.clone() {
                    Ok(result) => store.dispatch(StorageRequestSuccessAction { req_id, result }),
                    Err(error) => store.dispatch(StorageRequestErrorAction { req_id, error }),
                };
            } else {
                match &resp.result {
                    Ok(result) => match result {
                        StorageResponseSuccess::StateSnapshotPutSuccess(action_id) => {
                            let action_id = *action_id;
                            store.dispatch(StorageStateSnapshotCreateSuccessAction { action_id });
                        }
                        _ => return,
                    },
                    Err(result) => match result {
                        StorageResponseError::StateSnapshotPutError(action_id, error) => {
                            store.dispatch(StorageStateSnapshotCreateErrorAction {
                                action_id: *action_id,
                                error: error.clone(),
                            });
                        }
                        _ => return,
                    },
                };
            }
        }
        Action::StorageRequestError(action) => {
            store.dispatch(StorageRequestFinishAction {
                req_id: action.req_id,
            });
        }
        Action::StorageRequestSuccess(action) => {
            store.dispatch(StorageRequestFinishAction {
                req_id: action.req_id,
            });
        }
        _ => {}
    }
}
