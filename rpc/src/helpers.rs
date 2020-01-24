// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;

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
    pub protocol_data: HashMap<String, Value>,
}

impl FullBlockInfo {
    pub fn new(val: &BlockApplied, chain_id: &str) -> Self {
        let header: &BlockHeader = &val.header().header;
        let predecessor = HashType::BlockHash.bytes_to_string(header.predecessor());
        let timestamp = ts_to_rfc3339(header.timestamp());
        let operations_hash = HashType::OperationListListHash.bytes_to_string(header.operations_hash());
        let fitness = header.fitness().iter().map(|x| hex::encode(&x)).collect();
        let context = HashType::ContextHash.bytes_to_string(header.context());
        let hash = HashType::BlockHash.bytes_to_string(&val.header().hash);
        let json_data = val.json_data();

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
                protocol_data: serde_json::from_str(json_data.block_header_proto_json()).unwrap_or_default()
            },
            metadata: serde_json::from_str(json_data.block_header_proto_metadata_json()).unwrap_or_default(),
            operations: serde_json::from_str(json_data.operations_proto_metadata_json()).unwrap_or_default(),
        }
    }
}

impl Into<HashMap<String, Value>> for InnerBlockHeader {
    fn into(self) -> HashMap<String, Value> {
        let mut map: HashMap<String, Value> = HashMap::new();
        map.insert("level".to_string(), self.level.into());
        map.insert("proto".to_string(), self.proto.into());
        map.insert("predecessor".to_string(), self.predecessor.into());
        map.insert("timestamp".to_string(), self.timestamp.into());
        map.insert("validation_pass".to_string(), self.validation_pass.into());
        map.insert("operations_hash".to_string(), self.operations_hash.into());
        map.insert("fitness".to_string(), self.fitness.into());
        map.insert("context".to_string(), self.context.into());
        map.extend(self.protocol_data);
        map
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

#[derive(Serialize, Debug, Clone)]
pub enum RpcResponseData {
    EndorsingRights(Vec<EndorsingRight>),
    ErrorMsg(RpcErrorMsg),
}

#[derive(Serialize, Debug, Clone, Default)]
pub struct EndorsingRight {
    level: i64,
    delegate: String,
    slots: Vec<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    estimated_time: Option<String>
}

impl EndorsingRight {
    pub fn new(level: i64, delegate: String, slots: Vec<u8>, estimated_time: Option<String>) -> Self {
        Self {
            level,
            delegate: delegate.to_string(),
            slots,
            estimated_time,
        }
    }
}

#[derive(Serialize, Debug, Clone, Default)]
pub struct RpcErrorMsg {
    kind: String, // "permanent"
    id: String, // "proto.005-PsBabyM1.seed.unknown_seed"
    #[serde(skip_serializing_if = "Option::is_none")]
    missing_key: Option<MissingKey>,
    #[serde(skip_serializing_if = "Option::is_none")]
    oldest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    requested: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    latest: Option<String>,
}

impl RpcErrorMsg {
    pub fn new(
        kind: String, 
        id: String, 
        missing_key: Option<MissingKey>,
        oldest: Option<String>,
        requested: Option<String>,
        latest: Option<String>) -> Self {

        Self {
            kind: kind.to_string(),
            id: id.to_string(),
            missing_key,
            oldest,
            requested,
            latest,
        }
    }
}

#[derive(Serialize, Debug, Clone, Default)]
pub struct MissingKey {
    cycle: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    random_seed: Option<String>,
}

// cycle in which is given level
// level 0 (genesis block) is not part of any cycle (cycle 0 starts at level 1), hence the -1
pub fn cycle_from_level(level: i64, blocks_per_cycle: i64) -> i64 {
    (level - 1) / blocks_per_cycle
}

// the position of the block in its cycle
// level 0 (genesis block) is not part of any cycle (cycle 0 starts at level 1), hence the -1
pub fn level_position(level:i64, blocks_per_cycle:i64) -> i64 {
    let cycle_position = (level % blocks_per_cycle) - 1;
    if cycle_position < 0 { //for last block
        blocks_per_cycle - 1
    } else {
        cycle_position
    }
}