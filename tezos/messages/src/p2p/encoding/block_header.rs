// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use derive_builder::Builder;
use getset::{CopyGetters, Getters};
use serde::{Deserialize, Serialize};

use crate::Timestamp;

use super::fitness::Fitness;
use crypto::hash::{BlockHash, ContextHash, OperationListListHash};
use tezos_encoding::encoding::HasEncoding;
use tezos_encoding::nom::NomReader;
use tezos_encoding::{enc::BinWriter, types::Bytes};

use super::limits::{
    BLOCK_HEADER_MAX_SIZE, BLOCK_HEADER_PROTOCOL_DATA_MAX_SIZE, GET_BLOCK_HEADERS_MAX_LENGTH,
};

pub type Level = i32;

pub fn display_fitness(fitness: &Fitness) -> String {
    fitness.to_string()
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(
    Serialize, Deserialize, Debug, Eq, PartialEq, Getters, Clone, HasEncoding, NomReader, BinWriter,
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
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(
    Serialize, Deserialize, Debug, Eq, PartialEq, Getters, Clone, HasEncoding, NomReader, BinWriter,
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
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(
    Serialize,
    Deserialize,
    Eq,
    PartialEq,
    Clone,
    Builder,
    Getters,
    CopyGetters,
    HasEncoding,
    NomReader,
    BinWriter,
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
    timestamp: Timestamp,
    #[get_copy = "pub"]
    validation_pass: u8,
    #[get = "pub"]
    operations_hash: OperationListListHash,
    #[get = "pub"]
    fitness: Fitness,
    #[get = "pub"]
    context: ContextHash,

    #[get = "pub"]
    #[encoding(bounded = "BLOCK_HEADER_PROTOCOL_DATA_MAX_SIZE")]
    protocol_data: Bytes,

    #[get = "pub"]
    #[serde(skip)]
    #[builder(default)]
    #[encoding(hash)]
    hash: EncodingHash,
}

impl std::fmt::Debug for BlockHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BlockHeader")
            .field("level", &self.level)
            .field("proto", &self.proto)
            .field("predecessor", &self.predecessor)
            .field("timestamp", &self.timestamp)
            .field("validation_pass", &self.validation_pass)
            .field("operations_hash", &self.operations_hash)
            .field("fitness", &self.fitness)
            .field("context", &self.context)
            .field("protocol_data", &self.protocol_data)
            .finish()
    }
}

/// Optional 256-bit digest of encoded data
/// TODO https://viablesystems.atlassian.net/browse/TE-675
#[cfg_attr(
    feature = "fuzzing",
    derive(fuzzcheck::DefaultMutator, Serialize, Deserialize)
)]
#[derive(Clone, Debug, Eq, Default)]
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
