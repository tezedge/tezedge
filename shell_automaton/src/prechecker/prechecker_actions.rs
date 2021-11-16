// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crypto::hash::BlockHash;
use tezos_messages::p2p::encoding::operation::Operation;

use crate::rights::Slot;

use super::{Key, OperationDecodedContents, PrecheckerError, PrecheckerValidationError};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrecheckerPrecheckOperationAction {
    pub key: Key,
    pub operation: Operation,
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
pub struct PrecheckerGetEndorsingRightsAction {
    pub key: Key,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrecheckerEndorsingRightsReadyAction {
    pub key: Key,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrecheckerValidateEndorsementAction {
    pub key: Key,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrecheckerEndorsementValidationReadyAction {
    pub key: Key,
    pub result: Result<(), PrecheckerValidationError>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrecheckerErrorAction {
    pub key: Key,
    pub error: PrecheckerError,
}
