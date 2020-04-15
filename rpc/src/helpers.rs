// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;
use std::convert::TryInto;

use failure::bail;
use serde::Serialize;
use serde_json::Value;
use itertools::Itertools;

use crypto::hash::{BlockHash, HashType, ProtocolHash};
use shell::shell_channel::BlockApplied;
use storage::{BlockMetaStorage, BlockStorage, BlockStorageReader};
use storage::persistent::PersistentStorage;
use storage::skip_list::Bucket;
use tezos_messages::p2p::encoding::prelude::*;
use tezos_messages::ts_to_rfc3339;

use crate::ContextList;
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
    pub protocol: String,
    pub signature: String,
    pub priority: i64,
    // #[serde(skip_serializing_if = "Option::is_none")]
    // pub seed_nonce_hash: Option<String>,
    pub proof_of_work_nonce: String,
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
        let signature = header_data.get("signature").unwrap();
        let priority = header_data.get("priority").unwrap();
        let proof_of_work_nonce = header_data.get("proof_of_work_nonce").unwrap();
        // let seed_nonce_hash = header_data.get("seed_nonce_hash").unwrap().as_str();
        let proto_data: HashMap<String, Value> = serde_json::from_str(val.json_data().block_header_proto_metadata_json()).unwrap_or_default();
        let protocol = proto_data.get("protocol").unwrap();

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
            protocol: protocol.as_str().unwrap().to_string(),
            signature: signature.as_str().unwrap().to_string(),
            priority: priority.as_i64().unwrap(),
            //seed_nonce_hash,
            proof_of_work_nonce: proof_of_work_nonce.as_str().unwrap().to_string(),
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

// TODO: refactor errors
/// Struct is defining Error message response, there are different keys is these messages so only needed one are defined for each message
#[derive(Serialize, Debug, Clone)]
pub struct RpcErrorMsg {
    kind: String, // "permanent"
    id: String, // "proto.005-PsBabyM1.seed.unknown_seed"
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

// impl RpcErrorMsg {
//     pub fn new(
//         kind: String, 
//         id: String, 
//         missing_key: Option<Value>,
//         function: Option<String>,
//         oldest: Option<String>,
//         requested: Option<String>,
//         latest: Option<String>) -> Self {

//         Self {
//             kind: kind.to_string(),
//             id: id.to_string(),
//             missing_key,
//             function,
//             oldest,
//             requested,
//             latest,
//         }
//     }
// }

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
    // first check if 'head' string was provided as parameter and take hash from RpcCollectedStateRef
    let block_hash = if block_id == "head" {
        let state_read = state.read().unwrap();
        match state_read.current_head().as_ref() {
            Some(current_head) => {
                current_head.header().hash.clone()
            }
            None => bail!("head not initialized")
        }
    } else {
        let block_storage = BlockStorage::new(persistent_storage);
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
    list: ContextList,
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

    let protocol_hash: Vec<u8>;
    let constants: Vec<u8>;
    {
        let reader = list.read().unwrap();
        if let Some(Bucket::Exists(data)) = reader.get_key(level, &"protocol".to_string())? {
            protocol_hash = data;
        } else {
            panic!(format!("Protocol not found in context for block: {}, level: {}", block_id, level));
        }

        if let Some(Bucket::Exists(data)) = reader.get_key(level, &"data/v1/constants".to_string())? {
            constants = data;
        } else {
            panic!("Protocol constants not found in context for block: {}, level: {}, protocol_hash: {}", block_id, level, HashType::ProtocolHash.bytes_to_string(&protocol_hash));
        }
    };

    Ok(ContextProtocolParam {
        protocol_hash,
        constants_data: constants,
        level: level.try_into()?,
    })
}

pub(crate) fn get_context(level: &str, list: ContextList) -> Result<Option<HashMap<String, Bucket<Vec<u8>>>>, failure::Error> {
    let level = level.parse()?;
    {
        let storage = list.read().expect("poisoned storage lock");
        storage.get(level).map_err(|e| e.into())
    }
}

// #[inline]
// pub fn create_indexed_key_for_protocol_hash(prefix: &str, protocol_hash: &str) -> Result<Vec<String>, failure::Error> {
//     const INDEX_SIZE: usize = 6;
//     let mut key_vector = Vec::new();

//     // let address = contract_id_to_contract_address_for_index(contract_id)?;
//     let protocol_hash_bytes = HashType::ProtocolHash.string_to_bytes(protocol_hash)?;

//     let hashed = hex::encode(blake2b::digest_256(&protocol_hash_bytes));

//     key_vector.push(prefix.to_string());
//     for elem in (0..INDEX_SIZE * 2).step_by(2) {
//         key_vector.push(hashed[elem..elem + 2].to_string());
//     }

//     key_vector.push(hex::encode(protocol_hash_bytes));

//     Ok(key_vector)
// }

// TODO: rename this, couldn't think of a better type alias for a touple ("ed25519", "43a84d013b61b4c2cafe3fb89463329d7295a377")
//                                                                          curve               bytes
// to be used in SignaturePublicKeyHash::from_hex_hash_and_curve
pub type ComponentCurveAndHash = (String, String);

pub fn extract_curve_and_bytes(key: &str) -> Result<Option<ComponentCurveAndHash>, failure::Error> {
    let mut key_iter = key.split("/");

    // find the curve in the iterator and get his positon
    let curve = if let Some(c) = key_iter.find(|x| x == &"ed25519" || x == &"secp256k1" || x == &"p256") {
        c
    } else {
        return Ok(None);
    };

    // the sliced serilized component(pkh, protocol_hash, etc)
    // it directly follows the curve -> take the next 6 elements of the iterator
    let bytes = key_iter.take(6).join("");

    Ok(Some((curve.to_string(), bytes)))
}