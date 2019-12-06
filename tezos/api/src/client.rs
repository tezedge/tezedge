// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::fmt;

use serde::{Deserialize, Serialize};

use crypto::hash::*;
use tezos_encoding::binary_reader::BinaryReaderError;
use tezos_messages::p2p::binary_message::BinaryMessage;
use tezos_messages::p2p::encoding::prelude::*;

use crate::ffi::{OcamlStorageInitInfo, TestChain};

/// Struct represent init information about Tezos OCaml storage
#[derive(Serialize, Deserialize)]
pub struct TezosStorageInitInfo {
    pub chain_id: ChainId,
    pub test_chain: Option<TestChain>,
    pub genesis_block_header_hash: BlockHash,
    pub genesis_block_header: BlockHeader,
    pub current_block_header_hash: BlockHash,
    pub supported_protocol_hashes: Vec<ProtocolHash>,
}

impl TezosStorageInitInfo {
    pub fn new(storage_init_info: OcamlStorageInitInfo) -> Result<Self, BinaryReaderError> {
        Ok(TezosStorageInitInfo {
            chain_id: storage_init_info.chain_id,
            test_chain: storage_init_info.test_chain,
            genesis_block_header_hash: storage_init_info.genesis_block_header_hash,
            genesis_block_header: BlockHeader::from_bytes(storage_init_info.genesis_block_header)?,
            current_block_header_hash: storage_init_info.current_block_header_hash,
            supported_protocol_hashes: storage_init_info.supported_protocol_hashes,
        })
    }
}

impl fmt::Debug for TezosStorageInitInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let chain_hash_encoding = HashType::ChainId;
        let block_hash_encoding = HashType::BlockHash;
        write!(f, "TezosStorageInitInfo {{ chain_id: {}, genesis_block_header_hash: {}, current_block_header_hash: {} }}",
               chain_hash_encoding.bytes_to_string(&self.chain_id),
               block_hash_encoding.bytes_to_string(&self.genesis_block_header_hash),
               block_hash_encoding.bytes_to_string(&self.current_block_header_hash))
    }
}