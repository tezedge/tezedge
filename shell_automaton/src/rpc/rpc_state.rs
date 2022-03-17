// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::{BTreeMap, BTreeSet};

use crypto::hash::{BlockHash, ChainId, ContextHash, OperationListListHash, ProtocolHash};
use tezos_messages::{
    p2p::encoding::{block_header::Level, fitness::Fitness},
    Timestamp,
};

use crate::service::rpc_service::RpcId;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct RpcState {
    pub bootstrapped: Bootstrapped,
    pub valid_blocks: ValidBlocks,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct Bootstrapped {
    pub requests: BTreeSet<RpcId>,
    pub state: Option<BootstrapState>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BootstrapState {
    pub json: serde_json::Value,
    pub is_bootstrapped: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ValidBlocksQuery {
    pub chain_id: Option<ChainId>,
    pub protocol: Option<ProtocolHash>,
    pub next_protocol: Option<ProtocolHash>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct ValidBlocks {
    pub requests: BTreeMap<RpcId, ValidBlocksQuery>,
    pub state: Option<ValidBlockState>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ValidBlockState {
    chain_id: ChainId,
    hash: BlockHash,
    level: Level,
    proto: u8,
    predecessor: BlockHash,
    timestamp: Timestamp,
    validation_pass: u8,
    operations_hash: OperationListListHash,
    fitness: Fitness,
    context: ContextHash,
    protocol_data: Vec<u8>,
    // protocol_data: BoundedBytes<BLOCK_HEADER_PROTOCOL_DATA_MAX_SIZE>,
}
