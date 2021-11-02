// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use crate::request::RequestId;
use crate::service::storage_service::{
    StorageRequestPayload, StorageResponse, StorageResponseError, StorageResponseSuccess,
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageRequestCreateAction {
    pub payload: StorageRequestPayload,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageRequestInitAction {
    pub req_id: RequestId,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageRequestPendingAction {
    pub req_id: RequestId,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageResponseReceivedAction {
    pub response: StorageResponse,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageRequestErrorAction {
    pub req_id: RequestId,
    pub error: StorageResponseError,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageRequestSuccessAction {
    pub req_id: RequestId,
    pub result: StorageResponseSuccess,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageRequestFinishAction {
    pub req_id: RequestId,
}
