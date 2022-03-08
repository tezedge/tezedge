// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::{Action, ActionWithMeta, State};

use super::{StorageRequestState, StorageRequestStatus};

pub fn storage_request_reducer(state: &mut State, action: &ActionWithMeta) {
    match &action.action {
        Action::StorageRequestCreate(content) => {
            state.storage.requests.add(StorageRequestState {
                status: StorageRequestStatus::Idle {
                    time: action.time_as_nanos(),
                },
                payload: content.payload.clone(),
                requestor: content.requestor.clone(),
            });
        }
        Action::StorageRequestPending(content) => {
            if let Some(req) = state.storage.requests.get_mut(content.req_id) {
                match &req.status {
                    StorageRequestStatus::Idle { .. } => {
                        req.status = StorageRequestStatus::Pending {
                            time: action.time_as_nanos(),
                        };
                    }
                    _ => return,
                }
            }
        }
        Action::StorageRequestError(content) => {
            if let Some(req) = state.storage.requests.get_mut(content.req_id) {
                match &req.status {
                    StorageRequestStatus::Idle { time, .. }
                    | StorageRequestStatus::Pending { time, .. } => {
                        req.status = StorageRequestStatus::Error {
                            time: action.time_as_nanos(),
                            pending_since: *time,
                            error: content.error.clone(),
                        };
                    }
                    _ => return,
                }
            }
        }
        Action::StorageRequestSuccess(content) => {
            if let Some(req) = state.storage.requests.get_mut(content.req_id) {
                match &req.status {
                    StorageRequestStatus::Pending { time, .. } => {
                        req.status = StorageRequestStatus::Success {
                            time: action.time_as_nanos(),
                            pending_since: *time,
                            result: content.result.clone(),
                        };
                    }
                    _ => return,
                }
            }
        }
        Action::StorageRequestFinish(content) => {
            if let Some(req) = state.storage.requests.get(content.req_id) {
                match &req.status {
                    StorageRequestStatus::Error { .. } | StorageRequestStatus::Success { .. } => {
                        state.storage.requests.remove(content.req_id);
                    }
                    _ => return,
                }
            }
        }
        _ => {}
    }
}
