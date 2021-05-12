// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use derive_builder::Builder;
use getset::{CopyGetters, Getters};
use serde::{Deserialize, Serialize};

use crypto::hash::{BlockHash, ContextHash, HashType, OperationListListHash};
use tezos_encoding::encoding::{Encoding, Field, HasEncoding, HasEncodingTest, SchemaType};
use tezos_encoding::has_encoding_test;
use tezos_encoding::nom::NomReader;

use crate::cached_data;
use crate::p2p::binary_message::cache::BinaryDataCache;

use super::limits::{BLOCK_HEADER_MAX_SIZE, GET_BLOCK_HEADERS_MAX_LENGTH};

pub type Fitness = Vec<Vec<u8>>;
pub type Level = i32;

pub fn fitness_encoding() -> Encoding {
    Encoding::Split(Arc::new(|schema_type| match schema_type {
        SchemaType::Json => Encoding::dynamic(Encoding::list(Encoding::Bytes)),
        SchemaType::Binary => Encoding::dynamic(Encoding::list(Encoding::dynamic(Encoding::list(
            Encoding::Uint8,
        )))),
    }))
}

pub fn display_fitness(fitness: &Fitness) -> String {
    fitness
        .iter()
        .map(hex::encode)
        .collect::<Vec<String>>()
        .join("::")
}

#[derive(Serialize, Deserialize, Debug, Getters, Clone, HasEncoding, NomReader)]
pub struct BlockHeaderMessage {
    #[get = "pub"]
    block_header: BlockHeader,

    #[serde(skip_serializing)]
    #[encoding(skip)]
    body: BinaryDataCache,
}

cached_data!(BlockHeaderMessage, body);
has_encoding_test!(BlockHeaderMessage, BLOCK_HEADER_MESSAGE_ENCODING, {
    Encoding::Obj(
        "BlockHeaderMessage",
        vec![Field::new("block_header", BlockHeader::encoding_test().clone())],
    )
});

impl From<BlockHeader> for BlockHeaderMessage {
    fn from(block_header: BlockHeader) -> Self {
        BlockHeaderMessage {
            block_header,
            body: Default::default(),
        }
    }
}

impl From<BlockHeaderMessage> for BlockHeader {
    fn from(msg: BlockHeaderMessage) -> Self {
        msg.block_header
    }
}

// -----------------------------------------------------------------------------------------------
#[derive(Serialize, Deserialize, Debug, Getters, Clone, HasEncoding, NomReader)]
pub struct GetBlockHeadersMessage {
    #[get = "pub"]
    #[encoding(dynamic, list = "GET_BLOCK_HEADERS_MAX_LENGTH")]
    get_block_headers: Vec<BlockHash>,

    #[serde(skip_serializing)]
    #[encoding(skip)]
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
has_encoding_test!(
    GetBlockHeadersMessage,
    GET_BLOCK_HEADERS_MESSAGE_ENCODING,
    {
        Encoding::Obj(
            "GetBlockHeadersMessage",
            vec![Field::new(
                "get_block_headers",
                Encoding::dynamic(Encoding::bounded_list(
                    GET_BLOCK_HEADERS_MAX_LENGTH,
                    Encoding::Hash(HashType::BlockHash),
                )),
            )],
        )
    }
);

// -----------------------------------------------------------------------------------------------
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone, Builder, Getters, CopyGetters, HasEncoding, NomReader)]
#[encoding(bounded = "BLOCK_HEADER_MAX_SIZE")]
pub struct BlockHeader {
    #[get_copy = "pub"]
    #[encoding(builtin = "Int32")]
    level: Level,
    #[get_copy = "pub"]
    proto: u8,
    #[get = "pub"]
    predecessor: BlockHash,
    #[get_copy = "pub"]
    #[encoding(builtin = "Timestamp")]
    timestamp: i64,
    #[get_copy = "pub"]
    validation_pass: u8,
    #[get = "pub"]
    operations_hash: OperationListListHash,
    #[get = "pub"]
    #[encoding(composite(dynamic, list, dynamic, list, builtin = "Uint8"))]
    fitness: Fitness,
    #[get = "pub"]
    context: ContextHash,

    #[get = "pub"]
    #[encoding(list, builtin = "Uint8")]
    protocol_data: Vec<u8>,

    #[serde(skip_serializing)]
    #[builder(default)]
    #[encoding(skip)]
    body: BinaryDataCache,
}

cached_data!(BlockHeader, body);
has_encoding_test!(BlockHeader, BLOCK_HEADER_ENCODING, {
    Encoding::bounded(
        BLOCK_HEADER_MAX_SIZE,
        Encoding::Obj(
            "BlockHeader",
            vec![
                Field::new("level", Encoding::Int32),
                Field::new("proto", Encoding::Uint8),
                Field::new("predecessor", Encoding::Hash(HashType::BlockHash)),
                Field::new("timestamp", Encoding::Timestamp),
                Field::new("validation_pass", Encoding::Uint8),
                Field::new(
                    "operations_hash",
                    Encoding::Hash(HashType::OperationListListHash),
                ),
                Field::new("fitness", fitness_encoding()),
                Field::new("context", Encoding::Hash(HashType::ContextHash)),
                Field::new(
                    "protocol_data",
                    Encoding::Split(Arc::new(|schema_type| match schema_type {
                        SchemaType::Json => Encoding::Bytes,
                        SchemaType::Binary => Encoding::list(Encoding::Uint8),
                    })),
                ),
            ],
        ),
    )
});

#[cfg(test)]
mod test {
    use tezos_encoding::assert_encodings_match;

    use crate::p2p::binary_message::BinaryMessageNom;

    use super::*;

    #[test]
    fn test_block_header_encoding_schema() {
        assert_encodings_match!(BlockHeaderMessage);
    }

    #[test]
    fn test_decode_block_header_message_nom() {
        let data = hex::decode("00094F1F048D51777EF01C0106A09F747615CC72271A46EA75E097B48C7200CA2F1EAE6617000000005D7F495004C8626895CC82299089F495F7AD8864D1D3B0F364D497F1D175296B5F4A901EC80000001100000001000000000800000000012631B27A9F0E1DA2D2CA10202938298CFB1133D5F9A642F81E6697342263B6ECB621F10000000000032DB85C0E00961D14664ECBDF10CBE4DE7DD71096A4E1A177DB0890B13F0AB85999EB0D715E807BCA0438D3CEAA5C58560D60767F28A9E16326657FBE7FC8414FDE3C54A504").unwrap();
        let _message = <BlockHeaderMessage as BinaryMessageNom>::from_bytes(data).unwrap();
    }
}
