// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crypto::hash::{BlockHash, ChainId, OperationHash};
use tezos_api::ffi::{Applied, Errored, OperationProtocolDataJsonWithErrorListJson};
use tezos_messages::{
    p2p::encoding::{
        block_header::{BlockHeader, Level},
        operation::Operation,
    },
    protocol::SupportedProtocol,
};

use crate::{
    prechecker::{PrecheckerOperation, PrecheckerOperationState},
    rights::EndorsingRights,
    EnablingCondition, State,
};

use super::{
    EndorsementValidationError, Key, OperationDecodedContents, PrecheckerError,
    PrecheckerResponseError, SupportedProtocolState,
};

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrecheckerPrecheckOperationRequestAction {
    pub operation: Operation,
}

impl EnablingCondition<State> for PrecheckerPrecheckOperationRequestAction {
    fn is_enabled(&self, _state: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrecheckerPrecheckOperationResponseAction {
    pub response: PrecheckerPrecheckOperationResponse,
}

impl EnablingCondition<State> for PrecheckerPrecheckOperationResponseAction {
    fn is_enabled(&self, _state: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum PrecheckerPrecheckOperationResponse {
    /// The operation can be applied.
    Applied(PrecheckerApplied),
    /// The operation cannot be applied.
    Refused(PrecheckerErrored),
    /// Prechecker cannot decide if the operation is correct. Protocol based prevalidator is needed.
    Prevalidate(PrecheckerPrevalidate),
    /// Error occurred while prechecking the operation.
    Error(PrecheckerResponseError),
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrecheckerApplied {
    pub hash: OperationHash,
    pub operation_decoded_contents: OperationDecodedContents,
}

impl PrecheckerApplied {
    pub fn as_applied(&self) -> Applied {
        Applied {
            hash: self.hash.clone(),
            protocol_data_json: self.operation_decoded_contents.as_json().to_string(),
        }
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrecheckerErrored {
    pub hash: OperationHash,
    pub operation_decoded_contents: OperationDecodedContents,
    pub error: String,
}

impl PrecheckerErrored {
    pub fn is_endorsement(&self) -> bool {
        self.operation_decoded_contents.is_endorsement()
    }

    pub fn as_errored(&self) -> Errored {
        Errored {
            hash: self.hash.clone(),
            is_endorsement: Some(self.is_endorsement()),
            protocol_data_json_with_error_json: OperationProtocolDataJsonWithErrorListJson {
                protocol_data_json: self.operation_decoded_contents.as_json().to_string(),
                error_json: self.error.clone(),
            },
        }
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrecheckerPrevalidate {
    pub hash: OperationHash,
}

impl PrecheckerPrecheckOperationResponseAction {
    pub(super) fn valid(
        operation_hash: &OperationHash,
        operation_decoded_contents: OperationDecodedContents,
    ) -> Self {
        let applied = PrecheckerApplied {
            hash: operation_hash.clone(),
            operation_decoded_contents,
        };
        Self {
            response: PrecheckerPrecheckOperationResponse::Applied(applied),
        }
    }

    pub(super) fn reject(
        operation_hash: &OperationHash,
        operation_decoded_contents: OperationDecodedContents,
        error: String,
    ) -> Self {
        let errored = PrecheckerErrored {
            hash: operation_hash.clone(),
            error,
            operation_decoded_contents,
        };
        Self {
            response: PrecheckerPrecheckOperationResponse::Refused(errored),
        }
    }

    #[allow(dead_code)]
    pub(super) fn prevalidate(operation_hash: &OperationHash) -> Self {
        Self {
            response: PrecheckerPrecheckOperationResponse::Prevalidate(PrecheckerPrevalidate {
                hash: operation_hash.clone(),
            }),
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

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrecheckerPrecheckOperationInitAction {
    pub key: Key,
    pub operation: Operation,
    pub operation_binary_encoding: Vec<u8>,
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrecheckerGetProtocolVersionAction {
    pub key: Key,
}

impl EnablingCondition<State> for PrecheckerGetProtocolVersionAction {
    fn is_enabled(&self, state: &State) -> bool {
        matches!(
            state.prechecker.operations.get(&self.key),
            Some(PrecheckerOperation {
                state: PrecheckerOperationState::Init,
                ..
            })
        )
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrecheckerProtocolVersionReadyAction {
    pub key: Key,
}

impl EnablingCondition<State> for PrecheckerProtocolVersionReadyAction {
    fn is_enabled(&self, state: &State) -> bool {
        matches!(
            state.prechecker.operations.get(&self.key),
            Some(PrecheckerOperation {
                state: PrecheckerOperationState::PendingProtocolVersion { .. },
                ..
            })
        )
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrecheckerDecodeOperationAction {
    pub key: Key,
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrecheckerOperationDecodedAction {
    pub key: Key,
    pub contents: OperationDecodedContents,
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrecheckerWaitForBlockPrecheckedAction {
    pub key: Key,
    pub branch: BlockHash,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrecheckerBlockPrecheckedAction {
    pub key: Key,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrecheckerWaitForBlockAppliedAction {
    pub key: Key,
    pub branch: BlockHash,
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrecheckerBlockAppliedAction {
    pub key: Key,
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrecheckerGetEndorsingRightsAction {
    pub key: Key,
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrecheckerEndorsingRightsReadyAction {
    pub key: Key,
    pub endorsing_rights: EndorsingRights,
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrecheckerValidateEndorsementAction {
    pub key: Key,
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrecheckerEndorsementValidationAppliedAction {
    pub key: Key,
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrecheckerEndorsementValidationRefusedAction {
    pub key: Key,
    pub error: EndorsementValidationError,
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrecheckerProtocolNeededAction {
    pub key: Key,
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
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

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
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

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrecheckerSetNextBlockProtocolAction {
    pub proto: u8,
}

impl EnablingCondition<State> for PrecheckerSetNextBlockProtocolAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .prechecker
            .next_protocol
            .as_ref()
            .map_or(true, |(proto, _)| proto + 1 == self.proto)
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrecheckerQueryNextBlockProtocolAction {
    pub proto: u8,
    pub block_hash: BlockHash,
}

impl EnablingCondition<State> for PrecheckerQueryNextBlockProtocolAction {
    /// The action is enabled if protocol number exists but protocol data is not available.
    fn is_enabled(&self, state: &State) -> bool {
        state
            .prechecker
            .next_protocol
            .as_ref()
            .map_or(true, |(proto, state)| {
                (*proto == self.proto && matches!(state, SupportedProtocolState::None))
                    || proto + 1 == self.proto
            })
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrecheckerNextBlockProtocolReadyAction {
    pub block_hash: BlockHash,
    pub supported_protocol: SupportedProtocol,
}

impl EnablingCondition<State> for PrecheckerNextBlockProtocolReadyAction {
    fn is_enabled(&self, state: &State) -> bool {
        state.prechecker.next_protocol.as_ref().map_or(false, |(_, state)| matches!(state, SupportedProtocolState::Requesting(hash) if hash == &self.block_hash))
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrecheckerNextBlockProtocolErrorAction {
    pub block_hash: BlockHash,
    pub error: PrecheckerError,
}

impl EnablingCondition<State> for PrecheckerNextBlockProtocolErrorAction {
    fn is_enabled(&self, _state: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrecheckerPrecacheEndorsingRightsAction {
    pub current_head: BlockHash,
    pub level: Level,
}

impl EnablingCondition<State> for PrecheckerPrecacheEndorsingRightsAction {
    fn is_enabled(&self, _state: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrecheckerPruneOperationAction {
    pub key: Key,
}

impl EnablingCondition<State> for PrecheckerPruneOperationAction {
    fn is_enabled(&self, state: &State) -> bool {
        match state.prechecker.operations.get(&self.key) {
            Some(op) => match op.state {
                PrecheckerOperationState::Applied { .. }
                | PrecheckerOperationState::Refused { .. }
                | PrecheckerOperationState::ProtocolNeeded => true,
                _ => false,
            },
            None => false,
        }
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
impl EnablingCondition<State> for PrecheckerWaitForBlockPrecheckedAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}
impl EnablingCondition<State> for PrecheckerBlockPrecheckedAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}
impl EnablingCondition<State> for PrecheckerWaitForBlockAppliedAction {
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
