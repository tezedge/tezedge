// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crypto::hash::{BlockHash, ChainId};
use redux_rs::EnablingCondition;
use tezos_messages::p2p::encoding::block_header::BlockHeader;

use crate::{rights::EndorsingRights, State};

use super::{Key, OperationDecodedContents, PrecheckerError, PrecheckerInitError};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrecheckerPrecheckOperationAction {
    pub key: Key,
    pub block_hash: BlockHash,
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
pub struct PrecheckerInitErrorAction {
    pub error: PrecheckerInitError,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrecheckerErrorAction {
    pub key: Key,
    pub error: PrecheckerError,
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
impl EnablingCondition<State> for PrecheckerPrecheckOperationAction {
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
impl EnablingCondition<State> for PrecheckerInitErrorAction {
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
