// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;
use std::convert::TryFrom;
use std::convert::TryInto;

use failure::{bail, Fail, format_err};
use getset::Getters;
use serde::Serialize;
use serde_json::Value;

use crypto::blake2b;
use crypto::hash::{BlockHash, HashType};
use shell::shell_channel::BlockApplied;
use storage::{BlockMetaStorage, BlockStorage, BlockStorageReader};
use storage::num_from_slice;
use storage::persistent::PersistentStorage;
use storage::skip_list::Bucket;
use tezos_messages::p2p::encoding::prelude::*;

use crate::ContextList;
use crate::encoding::conversions::public_key_to_contract_id;
use crate::rpc_actor::RpcCollectedStateRef;
use crate::ts_to_rfc3339;

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

pub type ContextMap = HashMap<String, Bucket<Vec<u8>>>;

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
                protocol_data: serde_json::from_str(json_data.block_header_proto_json()).unwrap_or_default()
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

// TODO: refactor errors so this will be removed
// Enum defining possible response structures for RPC calls
// there is reason to have this structure because of format of error responses from ocaml node:
// [{"kind":"permanent","id":"proto.005-PsBabyM1.context.storage_error","missing_key":["cycle","4","random_seed"],"function":"get"}]
// [{"kind":"permanent","id":"proto.005-PsBabyM1.seed.unknown_seed","oldest":9,"requested":20,"latest":15}]
// if there have to be same response format then RpcErrorMsg is covering it
// this enum can be removed if errors are generated from error context directly in result_to_json_response function
#[derive(Serialize, Debug, Clone)]
pub enum RpcResponseData {
    EndorsingRights(Vec<EndorsingRight>),
    BakingRights(Vec<BakingRights>),
    // ErrorMsg(RpcErrorMsg),
}

/// Endorsing rights structure, final response look like Vec<EndorsingRight>
#[derive(Serialize, Debug, Clone)]
pub struct EndorsingRight {
    /// block level for which endorsing rights are generated
    level: i64,

    /// endorser contract id
    delegate: String,

    /// list of endorsement slots
    slots: Vec<u8>,

    /// estimated time of endorsement, is set to None if in past relative to block_id
    #[serde(skip_serializing_if = "Option::is_none")]
    estimated_time: Option<String>
}

impl EndorsingRight {
    /// Simple constructor to construct EndorsingRight
    pub fn new(level: i64, delegate: String, slots: Vec<u8>, estimated_time: Option<String>) -> Self {
        Self {
            level,
            delegate: delegate.to_string(),
            slots,
            estimated_time,
        }
    }
}

/// Object containing information about the baking rights 
#[derive(Serialize, Debug, Clone, Getters)]
pub struct BakingRights {
    /// block level for which baking rights are generated
    level: i64,

    /// baker contract id
    #[get = "pub(crate)"]
    delegate: String,

    /// baker priority to bake block
    priority: i64,

    /// estimated time of baking based on baking priority, is set to None if in past relative to block_id
    #[serde(skip_serializing_if = "Option::is_none")]
    estimated_time: Option<String>,
}

impl BakingRights {
    /// Simple constructor to construct BakingRights
    pub fn new(level: i64, delegate: String, priority: i64, estimated_time: Option<String>) -> Self{
        Self {
            level,
            delegate: delegate.to_string(),
            priority,
            estimated_time,
        }
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

/// Data from context DB for completing baking and endorsing rights
#[derive(Serialize, Debug, Clone, Getters)]
pub struct RightsContextData {
    /// Random seed for Tezos PRNG
    #[get = "pub(crate)"]
    random_seed: Vec<u8>,

    /// Number of last roll so Tezos PRNG will not overflow
    #[get = "pub(crate)"]
    last_roll: i32,

    /// List of rolls mapped to rollers contract id
    #[get = "pub(crate)"]
    rolls: HashMap<i32, String>,
}

impl RightsContextData {
    /// Simple constructor to create RightsContextData
    pub fn new(random_seed: Vec<u8>, last_roll: i32, rolls: HashMap<i32, String>) -> Self {
        Self {
            random_seed,
            last_roll,
            rolls,
        }
    }

    /// Get context data roll_snapshot, random_seed, last_roll and rolls from context list
    /// 
    /// # Arguments
    /// 
    /// * `parameters` - Parameters created by [RightsParams](RightsParams::parse_rights_parameters).
    /// * `constants` - Context constants used in baking and endorsing rights.
    /// * `list` - Context list handler.
    /// 
    /// Return RightsContextData.
    pub(crate) fn prepare_context_data_for_rights(parameters: RightsParams, constants: RightsConstants, list: ContextList) -> Result<Self, failure::Error> {
        // prepare constants that are used
        let blocks_per_cycle = *constants.blocks_per_cycle();
        let preserved_cycles = *constants.preserved_cycles();
        let blocks_per_roll_snapshot = *constants.blocks_per_roll_snapshot();
        
        // prepare parameters that are used
        let block_level = *parameters.block_level();
        let requested_level = *parameters.requested_level();

        // prepare cycle for which rollers are selected
        let requested_cycle = if let Some(cycle) = *parameters.requested_cycle() {
            cycle
        } else {
            cycle_from_level(requested_level, blocks_per_cycle)?
        };

        // get context list of block_id level as ContextMap
        let current_context = Self::get_context_as_hashmap(block_level.try_into()?, list.clone())?;
        
        // get index of roll snapshot
        let roll_snapshot: i16 = {
            let snapshot_key = format!("data/cycle/{}/roll_snapshot", requested_cycle);
            if let Some(Bucket::Exists(data)) = current_context.get(&snapshot_key) {
                num_from_slice!(data, 0, i16)
            } else { // key not found - prepare error for later processing
                return Err(format_err!("roll_snapshot"))
            }
        };

        let random_seed_key = format!("data/cycle/{}/random_seed", requested_cycle);
        let random_seed = {
            if let Some(Bucket::Exists(data)) = current_context.get(&random_seed_key) {
                data
            } else { // key not found - prepare error for later processing
                return Err(format_err!("random_seed"))
            }
        };

        // Snapshots of last_roll are listed from 0 same as roll_snapshot.
        let last_roll_key = format!("data/cycle/{}/last_roll/{}", requested_cycle, roll_snapshot);
        let last_roll = {
            if let Some(Bucket::Exists(data)) = current_context.get(&last_roll_key) {
                num_from_slice!(data, 0, i32)
            } else { // key not found - prepare error for later processing
                return Err(format_err!("last_roll"))
            }
        };
        
        // prepare context list from which rollers are selected
        // first prepare snapshot_level which is used access context list where are stored rollers for requested_cycle
        let snapshot_level = if requested_cycle < preserved_cycles+2 {
            block_level
        } else {
            let cycle_of_rolls = requested_cycle - preserved_cycles - 2;
            // to calculate order of snapshot add 1 to snapshot index (roll_snapshot)
            (cycle_of_rolls * blocks_per_cycle) + (((roll_snapshot + 1) as i64) * blocks_per_roll_snapshot) - 1
        };
        let roll_context = Self::get_context_as_hashmap(snapshot_level.try_into()?, list.clone())?;

        // get list of rolls from context list
        let context_rolls = if let Some(rolls) = Self::get_context_rolls(roll_context)? {
            rolls
        } else {
            return Err(format_err!("rolls"))
        };

        Ok(Self::new(
            random_seed.to_vec(),
            last_roll,
            context_rolls
        ))
    }

    /// get list of rollers from context list selected by snapshot level
    /// 
    /// # Arguments
    /// 
    /// * `context` - context list HashMap from [get_context_as_hashmap](RightsContextData::get_context_as_hashmap)
    /// 
    /// Return rollers for [RightsContextData.rolls](RightsContextData.rolls)
    fn get_context_rolls(context: ContextMap) -> Result<Option<HashMap<i32, String>>, failure::Error> {
        let data: ContextMap = context.into_iter()
            // .filter(|(k, _)| k.contains(&format!("data/rolls/owner/snapshot/{}/{}", cycle, snapshot)))
            .filter(|(k, _)| k.contains(&"data/rolls/owner/current"))  // TODO use line above after context db will contain all copied snapshots in block_id level of context list
            .collect();
            
        let mut roll_owners: HashMap<i32, String> = HashMap::new();

        // iterate through all the owners,the roll_num is the last component of the key, decode the value (it is a public key) to get the public key hash address (tz1...)
        for (key, value) in data.into_iter() {
            let roll_num = key.split('/').last().unwrap();

            // the values are public keys
            if let Bucket::Exists(pk) = value {
                let delegate = public_key_to_contract_id(pk)?;
                //let delegate = hex::encode(pk);
                roll_owners.insert(roll_num.parse().unwrap(), delegate);
            } else {
                continue;  // If the value is Deleted then is skipped and it go to the next iteration
            }
        }
        Ok(Some(roll_owners))
    }

    /// Get list of rollers from context list selected by snapshot level
    /// 
    /// # Arguments
    /// 
    /// * `level` - level to select from context list
    /// * `list` - context list handler
    /// 
    /// Return context list for given level as HashMap
    fn get_context_as_hashmap(level: usize, list: ContextList) -> Result<ContextMap, failure::Error> {
        // get the whole context
        let context = {
            let reader = list.read().unwrap();
            if let Ok(Some(ctx)) = reader.get(level) {
                ctx
            } else {
                bail!("Context not found")
            }
        };
        Ok(context)
    }
}
/// Set of parameters used to complete baking and endorsing rights
#[derive(Serialize, Debug, Clone, Getters)]
pub struct RightsParams {
    /// Id of a chain. Url path parameter 'chain_id'.
    #[get = "pub(crate)"]
    chain_id: String,

    /// Level (height) of block. Parsed from url path parameter 'block_id'.
    #[get = "pub(crate)"]
    block_level: i64,

    /// Header timestamp of block. Parsed from url path parameter 'block_id'.
    #[get = "pub(crate)"]
    block_timestamp: i64,

    /// Contract id to filter output by delegate. Url query parameter 'delegate'.
    #[get = "pub(crate)"]
    requested_delegate: Option<String>,

    /// Cycle for whitch all rights will be listed. Url query parameter 'cycle'.
    #[get = "pub(crate)"]
    requested_cycle: Option<i64>,

    /// Level (height) of block for whitch all rights will be listed. Url query parameter 'level'.
    #[get = "pub(crate)"]
    requested_level: i64,
    
    /// Level to be displayed in output. Endorsing rights only.
    #[get = "pub(crate)"]
    display_level: i64,
    
    /// Level for estimated_time computation. Endorsing rights only.
    #[get = "pub(crate)"]
    timestamp_level: i64,

    /// Max priority to which baking rights are listed. Url query parameter 'max_priority'.
    #[get = "pub(crate)"]
    max_priority: i64,

    /// Indicate that baking rights for maximum priority should be listed. Url query parameter 'all'.
    #[get = "pub(crate)"]
    has_all: bool,
}

impl RightsParams {
    /// Simple constructor to create RightsParams
    pub fn new(
        chain_id: String, 
        block_level: i64, 
        block_timestamp: i64,
        requested_delegate: Option<String>,
        requested_cycle: Option<i64>,
        requested_level: i64,
        display_level: i64,
        timestamp_level: i64,
        max_priority: i64,
        has_all: bool) -> Self {

        Self {
            chain_id,
            block_level,
            block_timestamp,
            requested_delegate,
            requested_cycle,
            requested_level,
            display_level,
            timestamp_level,
            max_priority,
            has_all,
        }
    }

    /// Prepare baking and endorsing rights parameters
    /// 
    /// # Arguments
    /// 
    /// * `param_chain_id` - Url path parameter 'chain_id'.
    /// * `param_level` - Url query parameter 'level'.
    /// * `param_delegate` - Url query parameter 'delegate'.
    /// * `param_cycle` - Url query parameter 'cycle'.
    /// * `param_max_priority` - Url query parameter 'max_priority'.
    /// * `param_has_all` - Url query parameter 'all'.
    /// * `block_level` - Block level from block_id.
    /// * `rights_constants` - Context constants used in baking and endorsing rights.
    /// * `context_list` - Context list handler.
    /// * `persistent_storage` - Persistent storage handler.
    /// * `is_baking_rights` - flag to identify if are parsed baking or endorsing rights
    /// 
    /// Return RightsParams
    pub(crate) fn parse_rights_parameters(
        param_chain_id: &str,
        param_level: Option<&str>,
        param_delegate: Option<&str>,
        param_cycle: Option<&str>,
        param_max_priority: Option<&str>,
        param_has_all: bool,
        block_level: i64,
        rights_constants: &RightsConstants,
        persistent_storage: &PersistentStorage,
        is_baking_rights: bool
    ) -> Result<Self, failure::Error> {
        let preserved_cycles = *rights_constants.preserved_cycles();
        let blocks_per_cycle = *rights_constants.blocks_per_cycle();

        // this is the cycle of block_id level
        let current_cycle = cycle_from_level(block_level, blocks_per_cycle)?;

        // endorsing rights only: display_level is here because of corner case where all levels < 1 are computed as level 1 but oputputed as they are
        let mut display_level: i64 = block_level;
        // endorsing rights only: timestamp_level is the base level for timestamp computation is taken from last known block (block_id) timestamp + time_between_blocks[0] 
        let mut timestamp_level:i64 = block_level;
        // Check the param_level, if there is a level specified validate it, if no set it to the block_level
        // requested_level is used to get data from context list
        let requested_level: i64 = match param_level {
            Some(level) => {
                let level = level.parse()?;
                // check the bounds for the requested level (if it is in the previous/next preserved cycles)
                Self::validate_cycle(cycle_from_level(level, blocks_per_cycle)?, current_cycle, preserved_cycles)?;
                // endorsing rights: display level is always same as level requested
                display_level = level;
                // endorsing rights: to compute timestamp for level parameter there need to be taken timestamp of previous block
                timestamp_level = level-1;
                // there can be requested also negative level, this is not an error but it is handled same as level 1 would be requested
                Self::get_valid_level(level)
            },
            // here is the main difference between endorsing and baking rights which data are loaded from context list based on level
            None => if is_baking_rights {
                    block_level + 1
                } else {
                    block_level
                }
        };

        // validate requested cycle
        let requested_cycle = match param_cycle {
            Some(val) => Some(Self::validate_cycle(val.parse()?, current_cycle, preserved_cycles)?),
            None => None
        };

        // set max_priority from param value or default
        let max_priority = match param_max_priority {
            Some(val) => val.parse()?,
            None => 64
        };

        // get block header timestamp form block_id
        let block_timestamp = get_block_timestamp_by_level(block_level.try_into()?, persistent_storage)?;

        Ok(Self::new(
            param_chain_id.to_string(),
            block_level,
            block_timestamp,
            param_delegate.map(String::from),
            requested_cycle,
            requested_level,
            display_level, // endorsing rights only
            timestamp_level, // endorsing rights only
            max_priority,
            param_has_all
        ))
    }

    /// Compute estimated time for endorsing rights
    /// 
    /// # Arguments
    /// 
    /// * `constants` - Context constants used in baking and endorsing rights.
    /// * `level` - Optional level for which is timestamp computed.
    /// 
    /// If level is not provided [timestamp_level](`RightsParams.timestamp_level`) is used fro computation
    #[inline]
    pub fn get_estimated_time(&self, constants: &RightsConstants, level: Option<i64>) -> Option<String> {
        // if is cycle then level is provided as parameter else use prepared timestamp_level
        let timestamp_level = level.unwrap_or(self.timestamp_level);

        //check if estimated time is computed and convert from raw epoch time to rfc3339 format
        if self.block_level <= timestamp_level {
            let est_timestamp = ((timestamp_level - self.block_level).abs() as i64 * constants.time_between_blocks()[0]) + self.block_timestamp;
            Some(ts_to_rfc3339(est_timestamp))
        } else {
            None
        }
    }

    /// In Ocaml Tezos node there can be negative level provided as query parameter, it is not handled as error but it will instead return endorsing rights for level 1
    #[inline]
    fn get_valid_level(level: i64) -> i64 {
        // for all reuqested negative levels use level 1
        if level < 1 {
            1
        } else {
            level
        }
    }

    /// Validate if cycle requested as url query parameter (cycle or level) is available in context list by checking preserved_cycles constant
    #[inline]
    fn validate_cycle(requested_cycle: i64, current_cycle: i64, preserved_cycles: i64) -> Result<i64, failure::Error> {
        if (requested_cycle - current_cycle).abs() <= preserved_cycles {
            Ok(requested_cycle)
        } else {
            bail!("Requested cycle out of bounds") //TODO: prepare cycle error
        }
    }
}

/// Context constants used in baking and endorsing rights
#[derive(Serialize, Debug, Clone, Getters)]
pub struct RightsConstants {
    #[get = "pub(crate)"]
    blocks_per_cycle: i64,
    #[get = "pub(crate)"]
    preserved_cycles: i64,
    #[get = "pub(crate)"]
    nonce_length: i64,
    #[get = "pub(crate)"]
    time_between_blocks: Vec<i64>,
    #[get = "pub(crate)"]
    blocks_per_roll_snapshot: i64,
    #[get = "pub(crate)"]
    endorsers_per_block: i64,
}

impl RightsConstants {
    /// simple constructor to create RightsConstants
    pub fn new(
        blocks_per_cycle: i64,
        preserved_cycles: i64,
        nonce_length: i64,
        time_between_blocks: Vec<i64>,
        blocks_per_roll_snapshot: i64,
        endorsers_per_block: i64
    ) -> Self {

        Self {
            blocks_per_cycle,
            preserved_cycles,
            nonce_length,
            time_between_blocks,
            blocks_per_roll_snapshot,
            endorsers_per_block,
        }
    }
}

/// Struct used in endorsing rights to map endorsers to endorsement slots before they can be ordered and completed
#[derive(Serialize, Debug, Clone, Getters)]
pub struct EndorserSlots {
    /// endorser contract id
    #[get = "pub(crate)"]
    contract_id: String,

    /// Orderer vector of endorsement slots
    #[get = "pub(crate)"]
    slots: Vec<u8>
}

impl EndorserSlots {
    /// Simple constructor that return EndorserSlots
    pub fn new(contract_id: String, slots: Vec<u8>) -> Self {
        Self {
            contract_id,
            slots
        }
    }

    /// Push endorsing slot to slots
    pub fn push_to_slot(&mut self, slot: u8) {
        self.slots.push(slot);
    }
}


/// Return cycle in which is given level
/// 
/// # Arguments
/// 
/// * `level` - level to specify cycle for
/// * `blocks_per_cycle` - context constant
/// 
/// Level 0 (genesis block) is not part of any cycle (cycle 0 starts at level 1),
/// hence the blocks_per_cycle - 1 for last cycle block.
pub fn cycle_from_level(level: i64, blocks_per_cycle: i64) -> Result<i64, failure::Error> {
    // check if blocks_per_cycle is not 0 to prevent panic
    if blocks_per_cycle > 0 {
        Ok((level - 1) / blocks_per_cycle)
    } else {
        bail!("wrong value blocks_per_cycle={}", blocks_per_cycle)
    }
}

/// Return the position of the block in its cycle
/// 
/// # Arguments
/// 
/// * `level` - level to specify cycle for
/// * `blocks_per_cycle` - context constant
/// 
/// Level 0 (genesis block) is not part of any cycle (cycle 0 starts at level 1),
/// hence the blocks_per_cycle - 1 for last cycle block.
pub fn level_position(level:i64, blocks_per_cycle:i64) -> Result<i64, failure::Error> {
    // check if blocks_per_cycle is not 0 to prevent panic
    if blocks_per_cycle <= 0 {
        bail!("wrong value blocks_per_cycle={}", blocks_per_cycle);
    }
    let cycle_position = (level % blocks_per_cycle) - 1;
    if cycle_position < 0 { //for last block
        Ok(blocks_per_cycle - 1)
    } else {
        Ok(cycle_position)
    }
}

/// Enum defining Tezos PRNG possible error
#[derive(Debug, Fail)]
pub enum TezosPRNGError {
    #[fail(display = "Value of bound(last_roll) not correct: {} bytes", bound)]
    BoundNotCorrect {
        bound: i32
    },
}

type RandomSeedState = Vec<u8>;
pub type TezosPRNGResult = Result<(i32, RandomSeedState), TezosPRNGError>;

/// Initialize Tezos PRNG
/// 
/// # Arguments
/// 
/// * `state` - RandomSeedState, initially the random seed.
/// * `nonce_size` - Nonce_length from current protocol constants.
/// * `blocks_per_cycle` - Blocks_per_cycle from current protocol context constants
/// * `use_string_bytes` - String converted to bytes, i.e. endorsing rights use b"level endorsement:".
/// * `level` - block level
/// * `offset` - For baking priority, for endorsing slot
/// 
/// Return first random sequence state to use in [get_prng_number](`get_prng_number`)
#[inline]
pub fn init_prng(cycle_data: &RightsContextData, constants: &RightsConstants, use_string_bytes: &[u8], level: i32, offset: i32) -> Result<RandomSeedState, failure::Error> {
    // a safe way to convert betwwen types is to use try_from
    let nonce_size = usize::try_from(*constants.nonce_length())?;
    let blocks_per_cycle = *constants.blocks_per_cycle();
    let state = cycle_data.random_seed(); 
    let zero_bytes: Vec<u8> = vec![0; nonce_size];

    // the position of the block in its cycle; has to be i32
    let cycle_position: i32 = level_position(level.into(), blocks_per_cycle)?.try_into()?;

    // take the state (initially the random seed), zero bytes, the use string and the blocks position in the cycle as bytes, merge them together and hash the result
    let rd = blake2b::digest_256(&merge_slices!(&state, &zero_bytes, use_string_bytes, &cycle_position.to_be_bytes())).to_vec();
    
    // take the 4 highest bytes and xor them with the priority/slot (offset)
    let higher = num_from_slice!(rd, 0, i32) ^ offset;
    
    // set the 4 highest bytes to the result of the xor operation
    let sequence = blake2b::digest_256(&merge_slices!(&higher.to_be_bytes(), &rd[4..]));
    
    Ok(sequence)
}

/// Get pseudo random nuber using Tezos PRNG
/// 
/// # Arguments
/// 
/// * `state` - RandomSeedState, initially the random seed.
/// * `bound` - Last possible roll nuber that have meaning to be generated taken from [RightsContextData.last_roll](`RightsContextData.last_roll`).
/// 
/// Return pseudo random generated roll number and RandomSeedState for next roll generation if the roll provided is missing from the roll list
#[inline]
pub fn get_prng_number(state: RandomSeedState, bound: i32) -> TezosPRNGResult {
    if bound < 1 {
        return Err(TezosPRNGError::BoundNotCorrect{bound: bound})
    }
    let v: i32;
        // Note: this part aims to be similar 
        // hash once again and take the 4 highest bytes and we got our random number
        let mut sequence = state;
        loop {
            let hashed = blake2b::digest_256(&sequence).to_vec();

            // computation for overflow check
            let drop_if_over = i32::max_value() - (i32::max_value() % bound);

            // 4 highest bytes
            let r = num_from_slice!(hashed, 0, i32).abs();

            // potentional overflow, keep the state of the generator and do one more iteration
            sequence = hashed;
            if r >= drop_if_over {
                continue;
            // use the remainder(mod) operation to get a number from a desired interval
            } else {
                v = r % bound;
                break;
            };
        }
        Ok((v.into(), sequence))
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
            Ok(value) =>  match block_storage.get_by_block_level(value)? {
                Some(current_head) => current_head.hash,
                None =>  bail!("block not found in db by level {}", block_id)
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
pub(crate) fn get_block_timestamp_by_level(level: i32,  persistent_storage: &PersistentStorage) -> Result<i64, failure::Error> {
    let block_storage = BlockStorage::new(persistent_storage);
    match block_storage.get_by_block_level(level)? {
        Some(current_head) => Ok(current_head.header.timestamp()),
        None => bail!("Block not found in db by level {}", level)
    }
}

/// Struct for the delegates and they voting power (in rolls)
#[derive(Serialize, Debug, Clone, Getters, Eq, Ord, PartialEq, PartialOrd)]
pub struct VoteListings {
    /// Public key hash (address, e.g tz1...)
    #[get = "pub(crate)"]
    pkh: String,

    /// Number of rolls the pkh owns
    #[get = "pub(crate)"]
    rolls: i32,
}

impl VoteListings {
    /// Simple constructor to construct VoteListings
    pub fn new(pkh: String, rolls: i32) -> Self{
        Self {
            pkh,
            rolls,
        }
    }
}