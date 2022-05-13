// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::convert::TryFrom;

use getset::Getters;
use hex::FromHexError;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crypto::{
    base58::FromBase58CheckError,
    hash::{BlockHash, OperationHash},
};
use tezos_encoding::encoding::HasEncoding;
use tezos_encoding::nom::NomReader;
use tezos_encoding::{enc::BinWriter, types::Bytes};

use super::limits::{GET_OPERATIONS_MAX_LENGTH, OPERATION_MAX_SIZE};

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(
    Serialize, Deserialize, Eq, PartialEq, Debug, Getters, Clone, HasEncoding, NomReader, BinWriter,
)]
pub struct OperationMessage {
    #[get = "pub"]
    operation: Operation,
}

impl From<Operation> for OperationMessage {
    fn from(operation: Operation) -> Self {
        Self { operation }
    }
}

impl From<OperationMessage> for Operation {
    fn from(msg: OperationMessage) -> Self {
        msg.operation
    }
}

// -----------------------------------------------------------------------------------------------
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(
    Clone, Serialize, Deserialize, Eq, PartialEq, Debug, HasEncoding, NomReader, BinWriter, Getters,
)]
pub struct Operation {
    #[get = "pub"]
    branch: BlockHash,
    #[encoding(bounded = "OPERATION_MAX_SIZE")]
    #[get = "pub"]
    data: Bytes,
}

impl Operation {
    pub fn new(branch: BlockHash, data: Bytes) -> Self {
        Self { branch, data }
    }
}

#[derive(Error, Debug)]
pub enum FromDecodedOperationError {
    #[error("Failed to decode from base58 string: {0}")]
    Base58(FromBase58CheckError),
    #[error("Failed to decode from hex string: {0}")]
    Hex(FromHexError),
}

impl From<FromBase58CheckError> for FromDecodedOperationError {
    fn from(source: FromBase58CheckError) -> Self {
        Self::Base58(source)
    }
}

impl From<FromHexError> for FromDecodedOperationError {
    fn from(source: FromHexError) -> Self {
        Self::Hex(source)
    }
}

impl TryFrom<DecodedOperation> for Operation {
    type Error = FromDecodedOperationError;
    fn try_from(dop: DecodedOperation) -> Result<Operation, FromDecodedOperationError> {
        Ok(Operation {
            branch: BlockHash::from_base58_check(&dop.branch)?,
            data: hex::decode(&dop.data)?.into(),
        })
    }
}

// -----------------------------------------------------------------------------------------------
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct DecodedOperation {
    branch: String,
    data: String,
}

impl From<Operation> for DecodedOperation {
    fn from(op: Operation) -> DecodedOperation {
        DecodedOperation {
            branch: op.branch().to_base58_check(),
            data: hex::encode(op.data()),
        }
    }
}

// -----------------------------------------------------------------------------------------------
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(
    Serialize, Deserialize, Debug, Eq, PartialEq, Getters, Clone, HasEncoding, NomReader, BinWriter,
)]
pub struct GetOperationsMessage {
    #[get = "pub"]
    #[encoding(dynamic, list = "GET_OPERATIONS_MAX_LENGTH")]
    get_operations: Vec<OperationHash>,
}

impl GetOperationsMessage {
    pub fn new(operations: Vec<OperationHash>) -> Self {
        Self {
            get_operations: operations,
        }
    }

    /// Creates possibly multiple messages for provided operations,
    /// respecting the limit for operations per message.
    pub fn from_operations(operations: Vec<OperationHash>) -> Vec<Self> {
        operations
            .chunks(GET_OPERATIONS_MAX_LENGTH)
            .map(|chunk| Self::new(chunk.to_vec()))
            .collect()
    }
}
