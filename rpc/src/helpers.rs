// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;
use std::str::FromStr;

use serde::Serialize;
use serde_json::Value;

use crypto::hash::HashType;
use shell::shell_channel::BlockApplied;
use tezos_messages::p2p::encoding::prelude::*;

use crate::ts_to_rfc3339;

#[derive(Serialize, Debug, Clone)]
/// Object containing information to recreate the full block information
pub struct FullBlockInfo {
    pub hash: String,
    pub chain_id: String,
    pub header: InnerBlockHeader,
    pub metadata: HashMap<String, Value>,
    pub operations: Vec<Vec<HashMap<String, Value>>>,
}

#[derive(Serialize, Debug, Clone)]
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

impl FullBlockInfo {
    pub fn new(val: &BlockApplied, chain_id: &str) -> Self {
        let header: &BlockHeader = &val.header().header;
        let json_data = val.json_data();
        let block_header_info: Option<BlockHeaderInfo> = json_data.block_header_proto_json().parse().ok();

        let predecessor = HashType::BlockHash.bytes_to_string(header.predecessor());
        let timestamp = ts_to_rfc3339(header.timestamp());
        let operations_hash = HashType::OperationListListHash.bytes_to_string(header.operations_hash());
        let fitness = header.fitness().iter().map(|x| hex::encode(&x)).collect();
        let context = HashType::ContextHash.bytes_to_string(header.context());
        let hash = HashType::BlockHash.bytes_to_string(&val.header().hash);
        let (priority, proof_of_work_nonce, signature) = if let Some(x) = block_header_info {
            (Some(x.priority), Some(x.proof_of_work_nonce), Some(x.signature))
        } else {
            (None, None, None)
        };

        Self {
            hash,
            chain_id: chain_id.into(),
            header: InnerBlockHeader {
                level: header.level(),
                proto: header.proto(),
                predecessor,
                timestamp,
                validation_pass: header.validation_pass(),
                operations_hash,
                fitness,
                context,
                priority,
                proof_of_work_nonce,
                signature,
            },
            metadata: serde_json::from_str(json_data.block_header_proto_metadata_json()).unwrap_or_default(),
            operations: serde_json::from_str(json_data.operations_proto_metadata_json()).unwrap_or_default(),
        }
    }
}

/// Structure containing basic information from block header
#[derive(Clone, Debug)]
pub struct BlockHeaderInfo {
    pub priority: i32,
    pub proof_of_work_nonce: String,
    pub signature: String,
}

impl FromStr for BlockHeaderInfo {
    type Err = serde_json::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let des: HashMap<&str, &str> = serde_json::from_str(s)?;
        Ok(Self {
            priority: if let Some(val) = des.get("priority") {
                val.parse().unwrap_or(0)
            } else {
                0
            },
            proof_of_work_nonce: (*des.get("proof_of_work_nonce").unwrap_or(&"")).into(),
            signature: (*des.get("signature").unwrap_or(&"")).into(),
        })
    }
}

/// Represents generic paged result.
#[derive(Debug, Serialize)]
pub struct PagedResult<C: Serialize> {
    /// Paged result data.
    data: C,
    /// ID of the next item if more items are available.
    /// If no more items are available then `None`.
    next_id: Option<u64>,
    /// Limit used in the request which produced this paged result.
    limit: usize,
}

impl<C> PagedResult<C>
    where
        C: Serialize
{
    pub fn new(data: C, next_id: Option<u64>, limit: usize) -> Self {
        PagedResult { data, next_id, limit }
    }
}

#[derive(Serialize, Debug, Clone, Default)]
pub struct EndorsingRight {
    level: i64,
    delegate: String,
    slots: Vec<u8>,
    estimated_time: String
}

impl EndorsingRight {
    pub fn new(level: i64, delegate: String, slots: Vec<u8>, estimated_time: String) -> Self {
        Self {
            level,
            delegate: delegate.to_string(),
            slots,
            estimated_time,
        }
    }
}