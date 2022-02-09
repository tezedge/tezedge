// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::bootstrap::BootstrapInitAction;
use crate::service::storage_service::{
    StorageRequestPayload, StorageResponseError, StorageResponseSuccess,
};
use crate::storage::request::StorageRequestCreateAction;
use crate::{Action, ActionWithMeta, Service, Store};

use super::{
    CurrentHeadRehydrateErrorAction, CurrentHeadRehydratePendingAction,
    CurrentHeadRehydrateSuccessAction, CurrentHeadRehydratedAction, CurrentHeadState,
};

pub fn current_head_effects<S>(store: &mut Store<S>, action: &ActionWithMeta)
where
    S: Service,
{
    match &action.action {
        Action::CurrentHeadRehydrateInit(_) => {
            let chain_id = store.state().config.chain_id.clone();
            store.dispatch(StorageRequestCreateAction {
                payload: StorageRequestPayload::CurrentHeadGet(chain_id),
            });
            let storage_req_id = store.state().storage.requests.last_added_req_id();
            store.dispatch(CurrentHeadRehydratePendingAction { storage_req_id });
        }
        Action::StorageResponseReceived(content) => {
            let target_req_id = match &store.state().current_head {
                CurrentHeadState::RehydratePending { storage_req_id, .. } => storage_req_id,
                _ => return,
            };
            if content
                .response
                .req_id
                .filter(|id| id.eq(target_req_id))
                .is_none()
            {
                return;
            }

            match &content.response.result {
                Ok(StorageResponseSuccess::CurrentHeadGetSuccess(head)) => {
                    store.dispatch(CurrentHeadRehydrateSuccessAction { head: head.clone() });
                }
                Err(StorageResponseError::CurrentHeadGetError(error)) => {
                    store.dispatch(CurrentHeadRehydrateErrorAction {
                        error: error.clone(),
                    });
                }
                _ => {}
            }
        }
        Action::CurrentHeadRehydrateSuccess(_) => {
            store.dispatch(CurrentHeadRehydratedAction {});
        }
        Action::CurrentHeadRehydrated(_) => {
            store.dispatch(BootstrapInitAction {});
        }
        _ => {}
    }
}
