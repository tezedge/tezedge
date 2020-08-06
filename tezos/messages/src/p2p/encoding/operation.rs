// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use getset::Getters;
use serde::{Deserialize, Serialize};

use crypto::hash::{BlockHash, HashType, OperationHash};
use tezos_encoding::encoding::{Encoding, Field, HasEncoding, SchemaType};
use tezos_encoding::has_encoding;

use crate::p2p::binary_message::cache::{BinaryDataCache, CachedData, CacheReader, CacheWriter};

#[derive(Serialize, Deserialize, PartialEq, Debug, Getters, Clone)]
pub struct OperationMessage {
    #[get = "pub"]
    operation: Operation,

    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

impl OperationMessage {
    pub fn new(operation: Operation) -> Self {
        Self {
            operation,
            body: Default::default(),
        }
    }
}

has_encoding!(OperationMessage, OPERATION_MESSAGE_ENCODING, {
    Encoding::Obj(vec![
        Field::new("operation", Operation::encoding().clone())
    ])
});

impl CachedData for OperationMessage {
    #[inline]
    fn cache_reader(&self) -> &dyn CacheReader {
        &self.body
    }

    #[inline]
    fn cache_writer(&mut self) -> Option<&mut dyn CacheWriter> {
        Some(&mut self.body)
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

has_encoding!(Operation, OPERATION_ENCODING, {
        Encoding::Obj(vec![
            Field::new("branch", Encoding::Hash(HashType::BlockHash)),
            Field::new("data", Encoding::Split(Arc::new(|schema_type|
                match schema_type {
                    SchemaType::Json => Encoding::Bytes,
                    SchemaType::Binary => Encoding::list(Encoding::Uint8)
                }
            )))
        ])
});

impl CachedData for Operation {
    #[inline]
    fn cache_reader(&self) -> &dyn CacheReader {
        &self.body
    }

    #[inline]
    fn cache_writer(&mut self) -> Option<&mut dyn CacheWriter> {
        Some(&mut self.body)
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
has_encoding!(GetOperationsMessage, GET_OPERATION_MESSAGE_ENCODING, {
        Encoding::Obj(vec![
            Field::new("get_operations", Encoding::dynamic(Encoding::list(Encoding::Hash(HashType::OperationHash)))),
        ])
});

impl CachedData for GetOperationsMessage {
    #[inline]
    fn cache_reader(&self) -> &dyn CacheReader {
        &self.body
    }

    #[inline]
    fn cache_writer(&mut self) -> Option<&mut dyn CacheWriter> {
        Some(&mut self.body)
    }
}