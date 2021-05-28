// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::convert::TryFrom;

use failure::Fail;
use getset::Getters;
use hex::FromHexError;
use serde::{Deserialize, Serialize};

use crypto::{
    base58::FromBase58CheckError,
    hash::{BlockHash, HashType, OperationHash},
};
use tezos_encoding::encoding::{Encoding, Field, HasEncoding, HasEncodingTest};
use tezos_encoding::has_encoding_test;
use tezos_encoding::nom::NomReader;

use crate::cached_data;
use crate::p2p::binary_message::cache::BinaryDataCache;

use super::limits::{GET_OPERATIONS_MAX_LENGTH, OPERATION_MAX_SIZE};

#[derive(Serialize, Deserialize, PartialEq, Debug, Getters, Clone, HasEncoding, NomReader)]
pub struct OperationMessage {
    #[get = "pub"]
    operation: Operation,

    #[serde(skip_serializing)]
    #[encoding(skip)]
    body: BinaryDataCache,
}

cached_data!(OperationMessage, body);
has_encoding_test!(OperationMessage, OPERATION_MESSAGE_ENCODING, {
    Encoding::Obj(
        "OperationMessage",
        vec![Field::new("operation", Operation::encoding_test().clone())],
    )
});

impl From<Operation> for OperationMessage {
    fn from(operation: Operation) -> Self {
        Self {
            operation,
            body: Default::default(),
        }
    }
}

impl From<OperationMessage> for Operation {
    fn from(msg: OperationMessage) -> Self {
        msg.operation
    }
}

// -----------------------------------------------------------------------------------------------
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug, HasEncoding, NomReader)]
pub struct Operation {
    branch: BlockHash,
    #[encoding(list = "OPERATION_MAX_SIZE")]
    data: Vec<u8>,

    #[serde(skip_serializing)]
    #[encoding(skip)]
    body: BinaryDataCache,
}

impl Operation {
    pub fn branch(&self) -> &BlockHash {
        &self.branch
    }

    pub fn data(&self) -> &Vec<u8> {
        &self.data
    }
}

#[derive(Fail, Debug)]
pub enum FromDecodedOperationError {
    #[fail(display = "Failed to decode from base58 string: {}", _0)]
    Base58(FromBase58CheckError),
    #[fail(display = "Failed to decode from hex string: {}", _0)]
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
            data: hex::decode(&dop.data)?,
            body: Default::default(),
        })
    }
}

cached_data!(Operation, body);
has_encoding_test!(Operation, OPERATION_ENCODING, {
    Encoding::Obj(
        "Operation",
        vec![
            Field::new("branch", Encoding::Hash(HashType::BlockHash)),
            Field::new(
                "data",
                Encoding::bounded_list(OPERATION_MAX_SIZE, Encoding::Uint8),
            ),
        ],
    )
});

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
#[derive(Serialize, Deserialize, Debug, Getters, Clone, HasEncoding, NomReader)]
pub struct GetOperationsMessage {
    #[get = "pub"]
    #[encoding(dynamic, list = "GET_OPERATIONS_MAX_LENGTH")]
    get_operations: Vec<OperationHash>,

    #[serde(skip_serializing)]
    #[encoding(skip)]
    body: BinaryDataCache,
}

impl GetOperationsMessage {
    pub fn new(operations: Vec<OperationHash>) -> Self {
        Self {
            get_operations: operations,
            body: Default::default(),
        }
    }
}

cached_data!(GetOperationsMessage, body);
has_encoding_test!(GetOperationsMessage, GET_OPERATION_MESSAGE_ENCODING, {
    Encoding::Obj(
        "GetOperationsMessage",
        vec![Field::new(
            "get_operations",
            Encoding::dynamic(Encoding::bounded_list(
                GET_OPERATIONS_MAX_LENGTH,
                Encoding::Hash(HashType::OperationHash),
            )),
        )],
    )
});

#[cfg(test)]
mod test {
    use tezos_encoding::assert_encodings_match;

    use super::*;

    #[test]
    fn test_operation_encoding_schema() {
        assert_encodings_match!(OperationMessage);
    }

    #[test]
    fn test_get_operations_encoding_schema() {
        assert_encodings_match!(GetOperationsMessage);
    }
}
