// Copyright (c) SimpleStaking and Tezos-RS Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use tezos_encoding::encoding::{Encoding, Field, HasEncoding, SchemaType};
use tezos_encoding::hash::{BlockHash, HashEncoding, HashType, OperationHash};

use crate::p2p::binary_message::cache::{BinaryDataCache, CacheReader, CacheWriter, CachedData};

#[derive(Serialize, Deserialize, Debug)]
pub struct OperationMessage {
    operation: Operation,

    #[serde(skip_serializing)]
    body: BinaryDataCache,
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
            Field::new("branch", Encoding::Hash(HashEncoding::new(HashType::BlockHash))),
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
#[derive(Serialize, Deserialize, Debug)]
pub struct GetOperationsMessage {
    get_operations: Vec<OperationHash>,

    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

impl HasEncoding for GetOperationsMessage {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("get_operations", Encoding::dynamic(Encoding::list(Encoding::Hash(HashEncoding::new(HashType::OperationHash))))),
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