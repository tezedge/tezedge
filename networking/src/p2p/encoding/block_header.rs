// Copyright (c) SimpleStaking and Tezos-RS Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use tezos_encoding::encoding::{Encoding, Field, HasEncoding, SchemaType};
use tezos_encoding::hash::{BlockHash, ContextHash, HashEncoding, HashType, OperationListListHash};

#[derive(Serialize, Deserialize, Debug)]
pub struct BlockHeaderMessage {
    pub block_header: BlockHeader
}

impl HasEncoding for BlockHeaderMessage {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("block_header", BlockHeader::encoding()),
        ])
    }
}

impl From<BlockHeader> for BlockHeaderMessage {
    fn from(block_header: BlockHeader) -> Self {
        BlockHeaderMessage { block_header }
    }
}

// -----------------------------------------------------------------------------------------------
#[derive(Serialize, Deserialize, Debug)]
pub struct GetBlockHeadersMessage {
    pub get_block_headers: Vec<BlockHash>,
}

impl HasEncoding for GetBlockHeadersMessage {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("get_block_headers", Encoding::dynamic(Encoding::list(Encoding::Hash(HashEncoding::new(HashType::BlockHash))))),
        ])
    }
}

// -----------------------------------------------------------------------------------------------
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct BlockHeader {
    pub level: i32,
    pub proto: u8,
    pub predecessor: BlockHash,
    pub timestamp: i64,
    pub validation_pass: u8,
    pub operations_hash: OperationListListHash,
    pub fitness: Vec<Vec<u8>>,
    pub context: ContextHash,
    pub protocol_data: Vec<u8>
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