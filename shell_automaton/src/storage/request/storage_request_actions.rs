// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use crate::request::RequestId;
use crate::service::storage_service::{
    StorageRequestPayload, StorageResponse, StorageResponseError, StorageResponseSuccess,
};
use crate::{EnablingCondition, State};

use super::StorageRequestor;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageRequestCreateAction {
    pub payload: StorageRequestPayload,
    pub requestor: StorageRequestor,
}

impl EnablingCondition<State> for StorageRequestCreateAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageRequestInitAction {
    pub req_id: RequestId,
}

impl EnablingCondition<State> for StorageRequestInitAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageRequestPendingAction {
    pub req_id: RequestId,
}

impl EnablingCondition<State> for StorageRequestPendingAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageResponseReceivedAction {
    pub response: StorageResponse,
    pub requestor: StorageRequestor,
}

impl EnablingCondition<State> for StorageResponseReceivedAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageRequestErrorAction {
    pub req_id: RequestId,
    pub error: StorageResponseError,
}

impl EnablingCondition<State> for StorageRequestErrorAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageRequestSuccessAction {
    pub req_id: RequestId,
    pub result: StorageResponseSuccess,
}

impl EnablingCondition<State> for StorageRequestSuccessAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageRequestFinishAction {
    pub req_id: RequestId,
}

impl EnablingCondition<State> for StorageRequestFinishAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}
