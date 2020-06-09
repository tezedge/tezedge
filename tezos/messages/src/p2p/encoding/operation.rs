// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::fmt;
use std::sync::Arc;

use failure::_core::fmt::Formatter;
use getset::Getters;
use serde::{Deserialize, Serialize};

use crypto::hash::{BlockHash, HashType, OperationHash};
use tezos_encoding::encoding::{Encoding, Field, HasEncoding, SchemaType};

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
            body: Default::default()
        }
    }
}

impl HasEncoding for OperationMessage {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("operation", Operation::encoding())
        ])
    }
}

impl CachedData for OperationMessage {
    #[inline]
    fn cache_reader(&self) -> & dyn CacheReader {
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

impl HasEncoding for Operation {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("branch", Encoding::Hash(HashType::BlockHash)),
            Field::new("data", Encoding::Split(Arc::new(|schema_type|
                match schema_type {
                    SchemaType::Json => Encoding::Bytes,
                    SchemaType::Binary => Encoding::list(Encoding::Uint8)
                }
            )))
        ])
    }
}

impl CachedData for Operation {
    #[inline]
    fn cache_reader(&self) -> & dyn CacheReader {
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
            body: Default::default()
        }
    }
}

impl HasEncoding for GetOperationsMessage {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("get_operations", Encoding::dynamic(Encoding::list(Encoding::Hash(HashType::OperationHash)))),
        ])
    }
}

impl CachedData for GetOperationsMessage {
    #[inline]
    fn cache_reader(&self) -> & dyn CacheReader {
        &self.body
    }

    #[inline]
    fn cache_writer(&mut self) -> Option<&mut dyn CacheWriter> {
        Some(&mut self.body)
    }
}

// -----------------------------------------------------------------------------------------------
/// Distinct
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum MempoolOperationType {
    Pending,
    KnownValid
}

impl MempoolOperationType {
    pub fn to_u8(&self) -> u8 {
        match self {
            MempoolOperationType::Pending => 0,
            MempoolOperationType::KnownValid => 1
        }
    }

    pub fn from_u8(num: u8) -> Result<Self, MempoolOperationTypeParseError> {
        match num {
            0 => Ok(MempoolOperationType::Pending),
            1 => Ok(MempoolOperationType::KnownValid),
            invalid_num => Err(MempoolOperationTypeParseError(invalid_num))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MempoolOperationTypeParseError(u8);

impl fmt::Display for MempoolOperationTypeParseError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_fmt(format_args!("Invalid value {}", self.0))
    }
}