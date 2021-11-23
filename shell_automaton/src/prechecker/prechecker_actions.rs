// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crypto::hash::{BlockHash, ChainId, OperationHash};
use tezos_messages::p2p::encoding::{block_header::BlockHeader, operation::Operation};

use crate::{rights::EndorsingRights, EnablingCondition, State};

use super::{Key, OperationDecodedContents, PrecheckerError, PrecheckerResponseError};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrecheckerPrecheckOperationRequestAction {
    pub operation: Operation,
}

impl EnablingCondition<State> for PrecheckerPrecheckOperationRequestAction {
    fn is_enabled(&self, _state: &State) -> bool {
        true
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrecheckerPrecheckOperationResponseAction {
    pub response: PrecheckerPrecheckOperationResponse,
}

impl EnablingCondition<State> for PrecheckerPrecheckOperationResponseAction {
    fn is_enabled(&self, _state: &State) -> bool {
        true
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum PrecheckerPrecheckOperationResponse {
    Valid(OperationHash),
    Reject(OperationHash),
    Prevalidate(Operation),
    Error(PrecheckerResponseError),
}

impl PrecheckerPrecheckOperationResponseAction {
    pub(super) fn valid(operation_hash: &OperationHash) -> Self {
        Self {
            response: PrecheckerPrecheckOperationResponse::Valid(operation_hash.clone()),
        }
    }

    pub(super) fn reject(operation_hash: &OperationHash) -> Self {
        Self {
            response: PrecheckerPrecheckOperationResponse::Reject(operation_hash.clone()),
        }
    }

    pub(super) fn prevalidate(operation: &Operation) -> Self {
        Self {
            response: PrecheckerPrecheckOperationResponse::Prevalidate(operation.clone()),
        }
    }

    pub(super) fn error<E>(error: E) -> Self
    where
        E: Into<PrecheckerResponseError>,
    {
        Self {
            response: PrecheckerPrecheckOperationResponse::Error(error.into()),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrecheckerPrecheckOperationInitAction {
    pub key: Key,
    pub operation: Operation,
    pub operation_binary_encoding: Vec<u8>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrecheckerDecodeOperationAction {
    pub key: Key,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrecheckerOperationDecodedAction {
    pub key: Key,
    pub contents: OperationDecodedContents,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrecheckerWaitForBlockApplicationAction {
    pub key: Key,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrecheckerBlockAppliedAction {
    pub key: Key,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrecheckerGetEndorsingRightsAction {
    pub key: Key,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrecheckerEndorsingRightsReadyAction {
    pub key: Key,
    pub endorsing_rights: EndorsingRights,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrecheckerValidateEndorsementAction {
    pub key: Key,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrecheckerEndorsementValidationReadyAction {
    pub key: Key,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrecheckerNotEndorsementAction {
    pub key: Key,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrecheckerErrorAction {
    pub key: Key,
    pub error: PrecheckerError,
}

impl PrecheckerErrorAction {
    pub(super) fn new<E>(key: Key, error: E) -> Self
    where
        E: Into<PrecheckerError>,
    {
        Self {
            key,
            error: error.into(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrecheckerCacheAppliedBlockAction {
    pub block_hash: BlockHash,
    pub chain_id: ChainId,
    pub block_header: BlockHeader,
}

impl EnablingCondition<State> for PrecheckerCacheAppliedBlockAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}
impl EnablingCondition<State> for PrecheckerPrecheckOperationInitAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}
impl EnablingCondition<State> for PrecheckerDecodeOperationAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}
impl EnablingCondition<State> for PrecheckerOperationDecodedAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}
impl EnablingCondition<State> for PrecheckerWaitForBlockApplicationAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}
impl EnablingCondition<State> for PrecheckerBlockAppliedAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}
impl EnablingCondition<State> for PrecheckerGetEndorsingRightsAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}
impl EnablingCondition<State> for PrecheckerEndorsingRightsReadyAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}
impl EnablingCondition<State> for PrecheckerValidateEndorsementAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}
impl EnablingCondition<State> for PrecheckerEndorsementValidationReadyAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}
impl EnablingCondition<State> for PrecheckerNotEndorsementAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}
impl EnablingCondition<State> for PrecheckerErrorAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}
