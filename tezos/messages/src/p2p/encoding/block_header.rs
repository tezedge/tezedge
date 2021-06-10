// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use derive_builder::Builder;
use getset::{CopyGetters, Getters};
use serde::{Deserialize, Serialize};

use crypto::hash::{BlockHash, ContextHash, OperationListListHash};
use tezos_encoding::enc::BinWriter;
use tezos_encoding::encoding::HasEncoding;
use tezos_encoding::nom::NomReader;

use super::limits::{
    BLOCK_HEADER_FITNESS_MAX_SIZE, BLOCK_HEADER_MAX_SIZE, BLOCK_HEADER_PROTOCOL_DATA_MAX_SIZE,
    GET_BLOCK_HEADERS_MAX_LENGTH,
};

pub type Fitness = Vec<Vec<u8>>;
pub type Level = i32;

pub fn display_fitness(fitness: &Fitness) -> String {
    fitness
        .iter()
        .map(hex::encode)
        .collect::<Vec<String>>()
        .join("::")
}

#[derive(
    Serialize,
    Deserialize,
    Debug,
    Getters,
    Clone,
    HasEncoding,
    NomReader,
    BinWriter,
    tezos_encoding::generator::Generated,
)]
pub struct BlockHeaderMessage {
    #[get = "pub"]
    block_header: BlockHeader,
}

impl From<BlockHeader> for BlockHeaderMessage {
    fn from(block_header: BlockHeader) -> Self {
        BlockHeaderMessage { block_header }
    }
}

impl From<BlockHeaderMessage> for BlockHeader {
    fn from(msg: BlockHeaderMessage) -> Self {
        msg.block_header
    }
}

// -----------------------------------------------------------------------------------------------
#[derive(
    Serialize,
    Deserialize,
    Debug,
    Getters,
    Clone,
    HasEncoding,
    NomReader,
    BinWriter,
    tezos_encoding::generator::Generated,
)]
pub struct GetBlockHeadersMessage {
    #[get = "pub"]
    #[encoding(dynamic, list = "GET_BLOCK_HEADERS_MAX_LENGTH")]
    get_block_headers: Vec<BlockHash>,
}

impl GetBlockHeadersMessage {
    pub fn new(get_block_headers: Vec<BlockHash>) -> Self {
        GetBlockHeadersMessage { get_block_headers }
    }
}

// -----------------------------------------------------------------------------------------------
#[derive(
    Serialize,
    Deserialize,
    PartialEq,
    Debug,
    Clone,
    Builder,
    Getters,
    CopyGetters,
    HasEncoding,
    NomReader,
    BinWriter,
    tezos_encoding::generator::Generated,
)]
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
    #[encoding(timestamp)]
    timestamp: i64,
    #[get_copy = "pub"]
    validation_pass: u8,
    #[get = "pub"]
    operations_hash: OperationListListHash,
    #[get = "pub"]
    #[encoding(composite(
        dynamic = "BLOCK_HEADER_FITNESS_MAX_SIZE",
        list,
        dynamic,
        list,
        builtin = "Uint8"
    ))]
    fitness: Fitness,
    #[get = "pub"]
    context: ContextHash,

    #[get = "pub"]
    #[encoding(
        bounded = "BLOCK_HEADER_PROTOCOL_DATA_MAX_SIZE",
        list,
        builtin = "Uint8"
    )]
    protocol_data: Vec<u8>,

    #[get = "pub"]
    #[serde(skip)]
    #[builder(default)]
    #[encoding(hash)]
    hash: EncodingHash,
}

/// Optional 256-bit digest of encoded data
/// TODO https://viablesystems.atlassian.net/browse/TE-675
#[derive(Clone, Debug)]
pub struct EncodingHash(pub Option<Vec<u8>>);

impl PartialEq for EncodingHash {
    fn eq(&self, other: &Self) -> bool {
        match (self.0.as_ref(), other.0.as_ref()) {
            (Some(v1), Some(v2)) => v1 == v2,
            _ => true,
        }
    }
}

impl EncodingHash {
    pub fn as_ref(&self) -> Option<&Vec<u8>> {
        self.0.as_ref()
    }
}

impl Default for EncodingHash {
    fn default() -> Self {
        Self(None)
    }
}

impl From<Vec<u8>> for EncodingHash {
    fn from(hash: Vec<u8>) -> Self {
        Self(Some(hash))
    }
}

#[cfg(test)]
mod test {
    use crypto::blake2b;

    use crate::p2p::binary_message::{BinaryRead, BinaryWrite};

    use super::*;

    #[test]
    fn test_decode_block_header_message_nom() {
        let data = hex::decode("00094F1F048D51777EF01C0106A09F747615CC72271A46EA75E097B48C7200CA2F1EAE6617000000005D7F495004C8626895CC82299089F495F7AD8864D1D3B0F364D497F1D175296B5F4A901EC80000001100000001000000000800000000012631B27A9F0E1DA2D2CA10202938298CFB1133D5F9A642F81E6697342263B6ECB621F10000000000032DB85C0E00961D14664ECBDF10CBE4DE7DD71096A4E1A177DB0890B13F0AB85999EB0D715E807BCA0438D3CEAA5C58560D60767F28A9E16326657FBE7FC8414FDE3C54A504").unwrap();
        let _message = BlockHeaderMessage::from_bytes(data).unwrap();
    }

    #[test]
    fn test_decode_block_header_message_hash() {
        let data = hex::decode("00094F1F048D51777EF01C0106A09F747615CC72271A46EA75E097B48C7200CA2F1EAE6617000000005D7F495004C8626895CC82299089F495F7AD8864D1D3B0F364D497F1D175296B5F4A901EC80000001100000001000000000800000000012631B27A9F0E1DA2D2CA10202938298CFB1133D5F9A642F81E6697342263B6ECB621F10000000000032DB85C0E00961D14664ECBDF10CBE4DE7DD71096A4E1A177DB0890B13F0AB85999EB0D715E807BCA0438D3CEAA5C58560D60767F28A9E16326657FBE7FC8414FDE3C54A504").unwrap();
        let hash = blake2b::digest_256(&data).unwrap();
        let message = BlockHeaderMessage::from_bytes(data).unwrap();
        let decode_hash = message.block_header().hash().as_ref().unwrap();
        assert_eq!(&hash, decode_hash);
        let encoded = message.as_bytes().unwrap();
        let encode_hash = blake2b::digest_256(&encoded).unwrap();
        assert_eq!(hash, encode_hash);
    }
}
