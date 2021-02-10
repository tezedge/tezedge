// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use getset::Getters;
use serde::{Deserialize, Serialize};

use crypto::hash::{BlockHash, HashType, OperationHash};
use tezos_encoding::encoding::{Encoding, Field, HasEncoding, SchemaType};
use tezos_encoding::has_encoding;

use crate::cached_data;
use crate::p2p::binary_message::cache::BinaryDataCache;

use super::limits::{GET_OPERATIONS_MAX_LENGTH, OPERATION_MAX_SIZE};

#[derive(Serialize, Deserialize, PartialEq, Debug, Getters, Clone)]
pub struct OperationMessage {
    #[get = "pub"]
    operation: Operation,

    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

cached_data!(OperationMessage, body);
has_encoding!(OperationMessage, OPERATION_MESSAGE_ENCODING, {
    Encoding::Obj(vec![Field::new("operation", Operation::encoding().clone())])
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
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct Operation {
    branch: BlockHash,
    data: Vec<u8>,

    #[serde(skip_serializing)]
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

impl From<DecodedOperation> for Operation {
    fn from(dop: DecodedOperation) -> Operation {
        Operation {
            branch: BlockHash::from_base58_check(&dop.branch).unwrap(),
            data: hex::decode(&dop.data).unwrap(),
            body: Default::default(),
        }
    }
}

cached_data!(Operation, body);
has_encoding!(Operation, OPERATION_ENCODING, {
    Encoding::Obj(vec![
        Field::new("branch", Encoding::Hash(HashType::BlockHash)),
        Field::new(
            "data",
            Encoding::Split(Arc::new(|schema_type| match schema_type {
                SchemaType::Json => Encoding::Bytes,
                SchemaType::Binary => Encoding::bounded_list(OPERATION_MAX_SIZE, Encoding::Uint8),
            })),
        ),
    ])
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
#[derive(Serialize, Deserialize, Debug, Getters, Clone)]
pub struct GetOperationsMessage {
    #[get = "pub"]
    get_operations: Vec<OperationHash>,

    #[serde(skip_serializing)]
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
has_encoding!(GetOperationsMessage, GET_OPERATION_MESSAGE_ENCODING, {
    Encoding::Obj(vec![Field::new(
        "get_operations",
        Encoding::dynamic(Encoding::bounded_list(
            GET_OPERATIONS_MAX_LENGTH,
            Encoding::Hash(HashType::OperationHash),
        )),
    )])
});
