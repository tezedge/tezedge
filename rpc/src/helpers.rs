// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;
use std::convert::TryInto;
use std::pin::Pin;

use chrono::Utc;
use failure::{bail, Fail};
use futures::Stream;
use futures::task::{Context, Poll};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crypto::hash::{BlockHash, chain_id_to_b58_string, HashType, ProtocolHash};
use shell::shell_channel::BlockApplied;
use storage::{BlockMetaStorage, BlockStorage, BlockStorageReader};
use storage::context_action_storage::ContextActionType;
use storage::context::{ContextApi, TezedgeContext};
use storage::persistent::{ContextMap, PersistentStorage};
use tezos_messages::p2p::encoding::prelude::*;
use tezos_messages::ts_to_rfc3339;

use crate::ContextList;
use crate::encoding::base_types::{TimeStamp, UniString};
use crate::rpc_actor::RpcCollectedStateRef;

#[macro_export]
macro_rules! merge_slices {
    ( $($x:expr),* ) => {{
        let mut res = vec![];
        $(
            res.extend_from_slice($x);
        )*
        res
    }}
}

/// Object containing information to recreate the full block information
#[derive(Serialize, Debug, Clone)]
pub struct FullBlockInfo {
    pub hash: String,
    pub chain_id: String,
    pub header: InnerBlockHeader,
    pub metadata: HashMap<String, Value>,
    pub operations: Vec<Vec<HashMap<String, Value>>>,
}

/// Object containing all block header information
#[derive(Serialize, Debug, Clone)]
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

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct HeaderContent {
    pub command: String,
    pub hash: String,
    pub fitness: Vec<String>,
    pub protocol_parameters: String,
}

/// Object containing information to recreate the block header information
#[derive(Serialize, Debug, Clone)]
pub struct BlockHeaderInfo {
    pub hash: String,
    pub chain_id: String,
    pub level: i32,
    pub proto: u8,
    pub predecessor: String,
    pub timestamp: String,
    pub validation_pass: u8,
    pub operations_hash: String,
    pub fitness: Vec<String>,
    pub context: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed_nonce_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proof_of_work_nonce: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<HeaderContent>,
}

/// Object containing information to recreate the block header shell information
#[derive(Serialize, Debug, Clone)]
pub struct BlockHeaderShellInfo {
    pub level: i32,
    pub proto: u8,
    pub predecessor: String,
    pub timestamp: String,
    pub validation_pass: u8,
    pub operations_hash: String,
    pub fitness: Vec<String>,
    pub context: String,
}

/// Object containing information to recreate the block header shell information
#[derive(Serialize, Debug, Clone)]
pub struct BlockHeaderMonitorInfo {
    pub hash: String,
    pub level: i32,
    pub proto: u8,
    pub predecessor: String,
    pub timestamp: String,
    pub validation_pass: u8,
    pub operations_hash: String,
    pub fitness: Vec<String>,
    pub context: String,
    pub protocol_data: String,
}

pub struct MonitorHeadStream {
    pub state: RpcCollectedStateRef,
    pub last_polled_timestamp: Option<TimeStamp>,
}

impl Stream for MonitorHeadStream {
    type Item = Result<String, serde_json::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Result<String, serde_json::Error>>> {
        // Note: the stream only ends on the client dropping the connection

        let state = self.state.read().unwrap();
        let last_update = if let TimeStamp::Integral(timestamp) = state.head_update_time() {
            *timestamp
        } else {
            cx.waker().wake_by_ref();
            return Poll::Pending;
        };
        let current_head = state.current_head().clone();
        let chain_id = state.chain_id().clone();

        // drop the immutable borrow so we can borrow self again as mutable
        // TODO: refactor this drop (remove if possible)
        drop(state);

        if let Some(TimeStamp::Integral(poll_time)) = self.last_polled_timestamp {
            if poll_time < last_update {
                // get the desired structure of the
                let current_head = current_head.as_ref().map(|current_head| {
                    let chain_id = chain_id_to_b58_string(&chain_id);
                    BlockHeaderInfo::new(current_head, &chain_id).to_monitor_header(current_head)
                });

                // serialize the struct to a json string to yield by the stream
                let mut head_string = serde_json::to_string(&current_head.unwrap())?;

                // push a newline character to the stream to imrove readability
                head_string.push('\n');

                self.last_polled_timestamp = Some(current_time_timestamp());

                // yield the serialized json
                return Poll::Ready(Some(Ok(head_string)));
            } else {
                cx.waker().wake_by_ref();
                return Poll::Pending;
            }
        } else {
            self.last_polled_timestamp = Some(current_time_timestamp());

            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
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
                protocol_data: serde_json::from_str(json_data.block_header_proto_json()).unwrap_or_default(),
            },
            metadata: serde_json::from_str(json_data.block_header_proto_metadata_json()).unwrap_or_default(),
            operations: serde_json::from_str(json_data.operations_proto_metadata_json()).unwrap_or_default(),
        }
    }
}

impl BlockHeaderInfo {
    pub fn new(val: &BlockApplied, chain_id: &str) -> Self {
        let header: &BlockHeader = &val.header().header;
        let predecessor = HashType::BlockHash.bytes_to_string(header.predecessor());
        let timestamp = ts_to_rfc3339(header.timestamp());
        let operations_hash = HashType::OperationListListHash.bytes_to_string(header.operations_hash());
        let fitness = header.fitness().iter().map(|x| hex::encode(&x)).collect();
        let context = HashType::ContextHash.bytes_to_string(header.context());
        let hash = HashType::BlockHash.bytes_to_string(&val.header().hash);
        let header_data: HashMap<String, Value> = serde_json::from_str(val.json_data().block_header_proto_json()).unwrap_or_default();
        let signature = header_data.get("signature").map(|val| val.as_str().unwrap().to_string());
        let priority = header_data.get("priority").map(|val| val.as_i64().unwrap());
        let proof_of_work_nonce = header_data.get("proof_of_work_nonce").map(|val| val.as_str().unwrap().to_string());
        let seed_nonce_hash = header_data.get("seed_nonce_hash").map(|val| val.as_str().unwrap().to_string());
        let proto_data: HashMap<String, Value> = serde_json::from_str(val.json_data().block_header_proto_metadata_json()).unwrap_or_default();
        let protocol = proto_data.get("protocol").map(|val| val.as_str().unwrap().to_string());

        let mut content: Option<HeaderContent> = None;
        if let Some(header_content) = header_data.get("content") {
            content = serde_json::from_value(header_content.clone()).unwrap();
        }

        Self {
            hash,
            chain_id: chain_id.into(),
            level: header.level(),
            proto: header.proto(),
            predecessor,
            timestamp,
            validation_pass: header.validation_pass(),
            operations_hash,
            fitness,
            context,
            protocol,
            signature,
            priority,
            seed_nonce_hash,
            proof_of_work_nonce,
            content,
        }
    }

    pub fn to_shell_header(&self) -> BlockHeaderShellInfo {
        BlockHeaderShellInfo {
            level: self.level,
            proto: self.proto,
            predecessor: self.predecessor.clone(),
            timestamp: self.timestamp.clone(),
            validation_pass: self.validation_pass,
            operations_hash: self.operations_hash.clone(),
            fitness: self.fitness.clone(),
            context: self.context.clone(),
        }
    }

    pub fn to_monitor_header(&self, block: &BlockApplied) -> BlockHeaderMonitorInfo {
        BlockHeaderMonitorInfo {
            hash: self.hash.clone(),
            level: self.level,
            proto: self.proto,
            predecessor: self.predecessor.clone(),
            timestamp: self.timestamp.clone(),
            validation_pass: self.validation_pass,
            operations_hash: self.operations_hash.clone(),
            fitness: self.fitness.clone(),
            context: self.context.clone(),
            protocol_data: hex::encode(block.header().header.protocol_data()),
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

#[allow(dead_code)]
impl<C> PagedResult<C>
    where
        C: Serialize
{
    pub fn new(data: C, next_id: Option<u64>, limit: usize) -> Self {
        PagedResult { data, next_id, limit }
    }
}

// TODO: refactor errors
/// Struct is defining Error message response, there are different keys is these messages so only needed one are defined for each message
#[derive(Serialize, Debug, Clone)]
pub struct RpcErrorMsg {
    kind: String,
    // "permanent"
    id: String,
    // "proto.005-PsBabyM1.seed.unknown_seed"
    #[serde(skip_serializing_if = "Option::is_none")]
    missing_key: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    function: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    oldest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    requested: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    latest: Option<String>,
}

#[derive(Serialize, Debug, Clone)]
pub struct Protocols {
    protocol: String,
    next_protocol: String,
}

impl Protocols {
    pub fn new(protocol: String, next_protocol: String) -> Self {
        Self {
            protocol,
            next_protocol,
        }
    }
}

// ---------------------------------------------------------------------
#[derive(Serialize, Debug, Clone)]
pub struct NodeVersion {
    version: Version,
    network_version: NetworkVersion,
    commit_info: CommitInfo,
}

#[derive(Serialize, Debug, Clone)]
pub struct CommitInfo {
    commit_hash: UniString,
    commit_date: UniString,
}

#[derive(Serialize, Debug, Clone)]
pub struct Version {
    major: i32,
    minor: i32,
    additional_info: String,
}

impl NodeVersion {
    pub fn new(network_version: &NetworkVersion) -> Self {
        let version_env: &'static str = env!("CARGO_PKG_VERSION");

        let version: Vec<String> = version_env.split(".").map(|v| v.to_string()).collect();

        Self {
            version: Version {
                major: version[0].parse().unwrap_or(0),
                minor: version[1].parse().unwrap_or(0),
                additional_info: "release".to_string(),
            },
            network_version: network_version.clone(),
            commit_info: CommitInfo {
                commit_hash: UniString::from(env!("GIT_HASH")),
                commit_date: UniString::from(env!("GIT_COMMIT_DATE")),
            },
        }
    }
}

/// Return block level based on block_id url parameter
/// 
/// # Arguments
/// 
/// * `block_id` - Url parameter block_id.
/// * `persistent_storage` - Persistent storage handler.
/// * `state` - Current RPC collected state (head).
/// 
/// If block_id is head return current head level
/// If block_id is level then return level as i64
/// if block_id is block hash string return level from BlockMetaStorage by block hash string
#[inline]
pub(crate) fn get_level_by_block_id(block_id: &str, persistent_storage: &PersistentStorage, state: &RpcCollectedStateRef) -> Result<Option<usize>, failure::Error> {
    // first try to parse level as number
    let level = match block_id.parse() {
        // block level was passed as parameter to block_id
        Ok(val) => Some(val),
        // block hash string or 'head' was passed as parameter to block_id
        Err(_e) => {
            let block_hash = get_block_hash_by_block_id(block_id, persistent_storage, state)?;
            let block_meta_storage: BlockMetaStorage = BlockMetaStorage::new(persistent_storage);
            if let Some(block_meta) = block_meta_storage.get(&block_hash)? {
                Some(block_meta.level() as usize)
            } else {
                None
            }
        }
    };

    Ok(level)
}

/// Get block has bytes from block hash or block level
/// # Arguments
/// 
/// * `block_id` - Url parameter block_id.
/// * `persistent_storage` - Persistent storage handler.
/// * `state` - Current RPC collected state (head).
/// 
/// If block_id is head return block hash byte string from current RpcCollectedStateRef
/// If block_id is level then return block hash byte string from BlockStorage by level
/// if block_id is block hash string return block hash byte string from BlockStorage by block hash string
#[inline]
pub(crate) fn get_block_hash_by_block_id(block_id: &str, persistent_storage: &PersistentStorage, state: &RpcCollectedStateRef) -> Result<BlockHash, failure::Error> {
    let block_storage = BlockStorage::new(persistent_storage);
    // first check if 'head' string was provided as parameter and take hash from RpcCollectedStateRef
    let block_hash = if block_id == "head" {
        let state_read = state.read().unwrap();
        match state_read.current_head().as_ref() {
            Some(current_head) => {
                current_head.header().hash.clone()
            }
            None => bail!("head not initialized")
        }
    } else if block_id == "genesis" {
        match block_storage.get_by_block_level(0)? {
            Some(genesis_block) => genesis_block.hash,
            None => bail!("Error getting genesis block from storage")
        }
    } else {
        // try to parse level as number
        match block_id.parse() {
            // block level was passed as parameter to block_id
            Ok(value) => match block_storage.get_by_block_level(value)? {
                Some(current_head) => current_head.hash,
                None => bail!("block not found in db by level {}", block_id)
            },
            // block hash string was passed as parameter to block_id
            Err(_e) => HashType::BlockHash.string_to_bytes(block_id)?
        }
    };

    Ok(block_hash)
}

#[inline]
pub(crate) fn get_action_types(action_types: &str) -> Vec<ContextActionType> {
    action_types.split(",")
        .filter_map(|x: &str| x.parse().ok())
        .collect()
}

/// Return block timestamp in epoch time format by block level
/// 
/// # Arguments
/// 
/// * `level` - Level of block.
/// * `state` - Current RPC state (head).
pub(crate) fn get_block_timestamp_by_level(level: i32, persistent_storage: &PersistentStorage) -> Result<i64, failure::Error> {
    let block_storage = BlockStorage::new(persistent_storage);
    match block_storage.get_by_block_level(level)? {
        Some(current_head) => Ok(current_head.header.timestamp()),
        None => bail!("Block not found in db by level {}", level)
    }
}

pub(crate) struct ContextProtocolParam {
    pub protocol_hash: ProtocolHash,
    pub constants_data: Vec<u8>,
    pub level: usize,
}


#[derive(Debug, Clone, Fail)]
pub enum ContextParamsError {
    #[fail(display = "Protocol not found in context for block: {}", _0)]
    NoProtocolForBlock(String),
    #[fail(display = "Protocol constants not found in context for block: {}", _0)]
    NoConstantsForBlock(String),

}


/// Get protocol and context constants as bytes from context list for desired block or level
///
/// # Arguments
///
/// * `block_id` - Url path parameter 'block_id', it contains string "head", block level or block hash.
/// * `opt_level` - Optionaly input block level from block_id if is already known to prevent double code execution.
/// * `list` - Context list handler.
/// * `persistent_storage` - Persistent storage handler.
/// * `state` - Current RPC collected state (head).
pub(crate) fn get_context_protocol_params(
    block_id: &str,
    opt_level: Option<i64>,
    persistent_storage: &PersistentStorage,
    state: &RpcCollectedStateRef) -> Result<ContextProtocolParam, failure::Error> {

    // first check if level is already known
    let level: usize = if let Some(l) = opt_level {
        l.try_into()?
    } else {
        // get level level by block_id
        if let Some(l) = get_level_by_block_id(block_id, persistent_storage, state)? {
            l
        } else {
            bail!("Level not found for block_id {}", block_id)
        }
    };

    let context = TezedgeContext::new(BlockStorage::new(&persistent_storage), persistent_storage.merkle());
    let ctx_hash = context.level_to_hash(level.try_into()?)?;

    let protocol_hash: Vec<u8>;
    let constants: Vec<u8>;
    {
        if let Some(data) = context.get_key_from_history(&ctx_hash, &vec!["protocol".to_string()])? {
            protocol_hash = data;
        } else {
            return Err(ContextParamsError::NoProtocolForBlock(block_id.to_string()).into());
        }

        if let Some(data) = context.get_key_from_history(&ctx_hash, &vec!["data".to_string(), "v1".to_string(), "constants".to_string()])? {
            constants = data;
        } else {
            return Err(ContextParamsError::NoConstantsForBlock(block_id.to_string()).into());
        }
    };

    Ok(ContextProtocolParam {
        protocol_hash,
        constants_data: constants,
        level: level.try_into()?,
    })
}

pub(crate) fn get_context(level: &str, list: ContextList) -> Result<Option<ContextMap>, failure::Error> {
    let level = level.parse()?;
    {
        let storage = list.read().expect("poisoned storage lock");
        storage.get(level).map_err(|e| e.into())
    }
}

pub(crate) fn current_time_timestamp() -> TimeStamp {
    TimeStamp::Integral(Utc::now().timestamp())
}
