// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use derive_builder::Builder;
use getset::{CopyGetters, Getters};
use serde::{Deserialize, Serialize};

use crypto::hash::{BlockHash, ContextHash, HashType, OperationListListHash};
use tezos_encoding::encoding::{Encoding, Field, HasEncoding, SchemaType};

use crate::p2p::binary_message::cache::{BinaryDataCache, CachedData, CacheReader, CacheWriter};

pub type Fitness = Vec<Vec<u8>>;

pub fn fitness_encoding() -> Encoding {
    Encoding::Split(Arc::new(|schema_type|
        match schema_type {
            SchemaType::Json => Encoding::dynamic(Encoding::list(Encoding::Bytes)),
            SchemaType::Binary => Encoding::dynamic(Encoding::list(
                Encoding::dynamic(Encoding::list(Encoding::Uint8))
            ))
        }
    ))
}

#[derive(Serialize, Deserialize, Debug, Getters, Clone)]
pub struct BlockHeaderMessage {
    #[get = "pub"]
    block_header: BlockHeader,

    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

impl HasEncoding for BlockHeaderMessage {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("block_header", BlockHeader::encoding()),
        ])
    }
}

impl CachedData for BlockHeaderMessage {
    #[inline]
    fn cache_reader(&self) -> &dyn CacheReader {
        &self.body
    }

    #[inline]
    fn cache_writer(&mut self) -> Option<&mut dyn CacheWriter> {
        Some(&mut self.body)
    }
}

impl From<BlockHeader> for BlockHeaderMessage {
    fn from(block_header: BlockHeader) -> Self {
        BlockHeaderMessage { block_header, body: Default::default() }
    }
}

// -----------------------------------------------------------------------------------------------
#[derive(Serialize, Deserialize, Debug, Getters, Clone)]
pub struct GetBlockHeadersMessage {
    #[get = "pub"]
    get_block_headers: Vec<BlockHash>,

    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

impl GetBlockHeadersMessage {
    pub fn new(get_block_headers: Vec<BlockHash>) -> Self {
        GetBlockHeadersMessage {
            get_block_headers,
            body: Default::default(),
        }
    }
}

impl HasEncoding for GetBlockHeadersMessage {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("get_block_headers", Encoding::dynamic(Encoding::list(Encoding::Hash(HashType::BlockHash)))),
        ])
    }
}

impl CachedData for GetBlockHeadersMessage {
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
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone, Builder, Getters, CopyGetters)]
pub struct BlockHeader {
    #[get_copy = "pub"]
    level: i32,
    #[get_copy = "pub"]
    proto: u8,
    #[get = "pub"]
    predecessor: BlockHash,
    #[get_copy = "pub"]
    timestamp: i64,
    #[get_copy = "pub"]
    validation_pass: u8,
    #[get = "pub"]
    operations_hash: OperationListListHash,
    #[get = "pub"]
    fitness: Fitness,
    #[get = "pub"]
    context: ContextHash,
    #[get = "pub"]
    protocol_data: Vec<u8>,

    #[serde(skip_serializing)]
    #[builder(default)]
    body: BinaryDataCache,
}

impl HasEncoding for BlockHeader {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("level", Encoding::Int32),
            Field::new("proto", Encoding::Uint8),
            Field::new("predecessor", Encoding::Hash(HashType::BlockHash)),
            Field::new("timestamp", Encoding::Timestamp),
            Field::new("validation_pass", Encoding::Uint8),
            Field::new("operations_hash", Encoding::Hash(HashType::OperationListListHash)),
            Field::new("fitness", fitness_encoding()),
            Field::new("context", Encoding::Hash(HashType::ContextHash)),
            Field::new("protocol_data", Encoding::Split(Arc::new(|schema_type|
                match schema_type {
                    SchemaType::Json => Encoding::Bytes,
                    SchemaType::Binary => Encoding::list(Encoding::Uint8)
                }
            )))
        ])
    }
}

impl CachedData for BlockHeader {
    #[inline]
    fn cache_reader(&self) -> &dyn CacheReader {
        &self.body
    }

    #[inline]
    fn cache_writer(&mut self) -> Option<&mut dyn CacheWriter> {
        Some(&mut self.body)
    }
}