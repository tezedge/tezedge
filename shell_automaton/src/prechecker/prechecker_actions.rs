// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crypto::hash::{BlockHash, ChainId, OperationHash};
use tezos_api::ffi::{Applied, Errored, OperationProtocolDataJsonWithErrorListJson};
use tezos_messages::p2p::encoding::{
    block_header::{BlockHeader, Level},
    operation::Operation,
};

use crate::{rights::EndorsingRights, EnablingCondition, State};

use super::{
    EndorsementValidationError, Key, OperationDecodedContents, PrecheckerError,
    PrecheckerResponseError,
};

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
    /// The operation can be applied.
    Applied(Applied),
    /// The operation cannot be applied.
    Refused(Errored),
    /// Prechecker cannot decide if the operation is correct. Protocol based prevalidator is needed.
    Prevalidate(Operation),
    /// Errro occurred while prechecking the operation.
    Error(PrecheckerResponseError),
}

impl PrecheckerPrecheckOperationResponseAction {
    pub(super) fn valid(operation_hash: &OperationHash, json: String) -> Self {
        let applied = Applied {
            hash: operation_hash.clone(),
            protocol_data_json: json,
        };
        Self {
            response: PrecheckerPrecheckOperationResponse::Applied(applied),
        }
    }

    pub(super) fn reject(
        operation_hash: &OperationHash,
        protocol_data: String,
        error: String,
    ) -> Self {
        let errored = Errored {
            hash: operation_hash.clone(),
            is_endorsement: Some(true),
            protocol_data_json_with_error_json: OperationProtocolDataJsonWithErrorListJson {
                error_json: error,
                protocol_data_json: protocol_data,
            },
        };
        Self {
            response: PrecheckerPrecheckOperationResponse::Refused(errored),
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
    pub level: Level,
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
pub struct PrecheckerEndorsementValidationAppliedAction {
    pub key: Key,
    pub protocol_data: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrecheckerEndorsementValidationRefusedAction {
    pub key: Key,
    pub protocol_data: String,
    pub error: EndorsementValidationError,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrecheckerProtocolNeededAction {
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
impl EnablingCondition<State> for PrecheckerEndorsementValidationAppliedAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}
impl EnablingCondition<State> for PrecheckerEndorsementValidationRefusedAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}
impl EnablingCondition<State> for PrecheckerProtocolNeededAction {
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
