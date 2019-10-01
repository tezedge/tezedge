// Copyright (c) SimpleStaking and Tezos-RS Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use tezos_encoding::encoding::{Encoding, Field, HasEncoding, SchemaType};
use tezos_encoding::hash::{BlockHash, ContextHash, HashEncoding, HashType, OperationListListHash};

use crate::p2p::binary_message::cache::{BinaryDataCache, CacheReader, CacheWriter, CachedData};

#[derive(Serialize, Deserialize, Debug)]
pub struct BlockHeaderMessage {
    pub block_header: BlockHeader,

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
    fn cache_reader(&self) -> & dyn CacheReader {
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
#[derive(Serialize, Deserialize, Debug, Default)]
pub struct GetBlockHeadersMessage {
    pub get_block_headers: Vec<BlockHash>,

    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

impl GetBlockHeadersMessage {
    pub fn new(get_block_headers: Vec<BlockHash>) -> Self {
        GetBlockHeadersMessage {
            get_block_headers,
            body: Default::default()
        }
    }
}

impl HasEncoding for GetBlockHeadersMessage {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("get_block_headers", Encoding::dynamic(Encoding::list(Encoding::Hash(HashEncoding::new(HashType::BlockHash))))),
        ])
    }
}

impl CachedData for GetBlockHeadersMessage {
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
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone, Default)]
pub struct BlockHeader {
    pub level: i32,
    pub proto: u8,
    pub predecessor: BlockHash,
    pub timestamp: i64,
    pub validation_pass: u8,
    pub operations_hash: OperationListListHash,
    pub fitness: Vec<Vec<u8>>,
    pub context: ContextHash,
    pub protocol_data: Vec<u8>,

    #[serde(skip_serializing)]
    pub body: BinaryDataCache,
}

impl HasEncoding for BlockHeader {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("level", Encoding::Int32),
            Field::new("proto", Encoding::Uint8),
            Field::new("predecessor", Encoding::Hash(HashEncoding::new(HashType::BlockHash))),
            Field::new("timestamp", Encoding::Timestamp),
            Field::new("validation_pass", Encoding::Uint8),
            Field::new("operations_hash", Encoding::Hash(HashEncoding::new(HashType::OperationListListHash))),
            Field::new("fitness", Encoding::Split(Arc::new(|schema_type|
                match schema_type {
                    SchemaType::Json => Encoding::dynamic(Encoding::list(Encoding::Bytes)),
                    SchemaType::Binary => Encoding::dynamic(Encoding::list(
                        Encoding::dynamic(Encoding::list(Encoding::Uint8))
                    ))
                }
            ))),
            Field::new("context", Encoding::Hash(HashEncoding::new(HashType::ContextHash))),
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
    fn cache_reader(&self) -> & dyn CacheReader {
        &self.body
    }

    #[inline]
    fn cache_writer(&mut self) -> Option<&mut dyn CacheWriter> {
        Some(&mut self.body)
    }
}