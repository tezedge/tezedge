// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use shell::shell_channel::BlockApplied;
use storage::BlockHeaderWithHash;
use tezos_encoding::hash::{HashEncoding, HashType};
use tezos_messages::p2p::encoding::prelude::*;

use crate::ts_to_rfc3339;

#[derive(Debug, Clone)]
/// Object containing information to recreate the full block information
pub struct FullBlockInfo {
    pub hash: String,
    pub chain_id: String,
    pub header: InnerBlockHeader,
    pub metadata: HashMap<String, String>,
    pub operations: Vec<OperationsForBlocksMessage>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
/// Object containing all block header information
pub struct InnerBlockHeader {
    pub level: i32,
    pub proto: u8,
    pub predecessor: String,
    pub timestamp: String,
    pub validation_pass: u8,
    pub operations_hash: String,
    pub fitness: Vec<String>,
    pub context: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proof_of_work_nonce: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

impl From<BlockApplied> for FullBlockInfo {
    fn from(val: BlockApplied) -> Self {
        let predecessor = HashEncoding::new(HashType::BlockHash).bytes_to_string(val.header.predecessor());
        let timestamp = ts_to_rfc3339(val.header.timestamp());
        let operations_hash = HashEncoding::new(HashType::OperationListListHash).bytes_to_string(val.header.predecessor());
        let fitness = val.header.fitness().iter().map(|x| hex::encode(&x)).collect();
        let context = HashEncoding::new(HashType::ContextHash).bytes_to_string(val.header.context());
        let hash = HashEncoding::new(HashType::BlockHash).bytes_to_string(&val.hash);
        let (priority, proof_of_work_nonce, signature) = if let Some(x) = val.block_header_info {
            (Some(x.priority), Some(x.proof_of_work_nonce), Some(x.signature))
        } else {
            (None, None, None)
        };

        Self {
            hash,
            chain_id: "".into(),
            header: InnerBlockHeader {
                level: val.header.level(),
                proto: val.header.proto(),
                predecessor,
                timestamp,
                validation_pass: val.header.validation_pass(),
                operations_hash,
                fitness,
                context,
                priority,
                proof_of_work_nonce,
                signature,
            },
            metadata: val.block_header_proto_info,
            operations: Vec::new(),
        }
    }
}

impl From<BlockHeaderWithHash> for FullBlockInfo {
    fn from(block: BlockHeaderWithHash) -> Self {
        let block: BlockApplied = block.into();
        block.into()
    }
}