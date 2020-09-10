// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use derive_builder::Builder;
use getset::{CopyGetters, Getters};
use serde::{Deserialize, Serialize};

use crypto::hash::{BlockHash, ContextHash, HashType, OperationListListHash};
use tezos_encoding::encoding::{Encoding, Field, HasEncoding, SchemaType};
use tezos_encoding::has_encoding;

use crate::cached_data;
use crate::p2p::binary_message::cache::BinaryDataCache;

pub type Fitness = Vec<Vec<u8>>;
pub type Level = i32;

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

pub fn display_fitness(fitness: &Fitness) -> String {
    fitness
        .iter()
        .map(|f| hex::encode(f))
        .collect::<Vec<String>>()
        .join("::")
}

#[derive(Serialize, Deserialize, Debug, Getters, Clone)]
pub struct BlockHeaderMessage {
    #[get = "pub"]
    block_header: BlockHeader,

    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

cached_data!(BlockHeaderMessage, body);
has_encoding!(BlockHeaderMessage, BLOCK_HEADER_MESSAGE_ENCODING, {
        Encoding::Obj(vec![
            Field::new("block_header", BlockHeader::encoding().clone()),
        ])
});

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

cached_data!(GetBlockHeadersMessage, body);
has_encoding!(GetBlockHeadersMessage, GET_BLOCK_HEADERS_MESSAGE_ENCODING, {
        Encoding::Obj(vec![
            Field::new("get_block_headers", Encoding::dynamic(Encoding::list(Encoding::Hash(HashType::BlockHash)))),
        ])
});

// -----------------------------------------------------------------------------------------------
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone, Builder, Getters, CopyGetters)]
pub struct BlockHeader {
    #[get_copy = "pub"]
    level: Level,
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

cached_data!(BlockHeader, body);
has_encoding!(BlockHeader, BLOCK_HEADER_ENCODING, {
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
});