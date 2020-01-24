// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;
use std::str::FromStr;

use getset::Getters;
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
/// Object containing information about the baking rights 
#[derive(Serialize, Debug, Clone, Getters)]
pub struct BakingRights {
    #[get = "pub(crate)"]
    level: i64,
    #[get = "pub(crate)"]
    delegate: String,
    #[get = "pub(crate)"]
    priority: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[get = "pub(crate)"]
    estimated_time: Option<String>,
}

impl BakingRights {
    pub fn new(level: i64, delegate: String, priority: i64, estimated_time: Option<String>) -> Self{
        Self {
            level,
            delegate: delegate.to_string(),
            priority,
            estimated_time,
        }
    }
}

/// Object containing information about the baking rights 
#[derive(Serialize, Debug, Clone, Getters)]
pub struct StakingRightsContextData {
    #[get = "pub(crate)"]
    head_level: i32,
    #[get = "pub(crate)"]
    block_level: i32,
    #[get = "pub(crate)"]
    roll_snapshot: i16,
    #[get = "pub(crate)"]
    last_roll: i32,
    #[get = "pub(crate)"]
    blocks_per_cycle: i64,
    #[get = "pub(crate)"]
    preserved_cycles: i64,
    #[get = "pub(crate)"]
    nonce_length: i64,
    #[get = "pub(crate)"]
    time_between_blocks: Vec<i64>,
    #[get = "pub(crate)"]
    random_seed: Vec<u8>,
    #[get = "pub(crate)"]
    rolls: HashMap<i64, String>,
}

impl StakingRightsContextData {
    pub fn new(head_level: i32, block_level: i32, roll_snapshot: i16, last_roll: i32, blocks_per_cycle: i64, preserved_cycles: i64, nonce_length: i64, time_between_blocks: Vec<i64>, random_seed: Vec<u8>, rolls: HashMap<i64, String>) -> Self{
        Self {
            head_level,
            block_level,
            roll_snapshot,
            last_roll,
            blocks_per_cycle,
            preserved_cycles,
            nonce_length,
            time_between_blocks,
            random_seed,
            rolls,
        }
    }
}