// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;
use std::convert::TryFrom;
use std::convert::TryInto;
use storage::num_from_slice;

use serde::Serialize;
use serde_json::Value;
use getset::Getters;
use failure::Fail;

use crypto::blake2b;
use crypto::hash::HashType;
use shell::shell_channel::BlockApplied;
use tezos_messages::p2p::encoding::prelude::*;

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
    ErrorMsg(RpcErrorMsg),
}

// endorsing rights structure, final response look like Vec<EndorsingRight>
#[derive(Serialize, Debug, Clone)]
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

// struct is defining Error message response, thre are different keys is these messages so only needed one are defined for each message
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

impl RpcErrorMsg {
    pub fn new(
        kind: String, 
        id: String, 
        missing_key: Option<Value>,
        function: Option<String>,
        oldest: Option<String>,
        requested: Option<String>,
        latest: Option<String>) -> Self {

        Self {
            kind: kind.to_string(),
            id: id.to_string(),
            missing_key,
            function,
            oldest,
            requested,
            latest,
        }
    }
}

#[derive(Serialize, Debug, Clone, Getters)]
pub struct CycleData {
    #[get = "pub(crate)"]
    random_seed: Vec<u8>,
    #[get = "pub(crate)"]
    last_roll: i32,
    #[get = "pub(crate)"]
    // rename to rolls
    // type alias
    rolls: HashMap<i32, String>,
}

impl CycleData {
    pub fn new(random_seed: Vec<u8>, last_roll: i32, rolls: HashMap<i32, String>) -> Self {
        Self {
            random_seed,
            last_roll,
            rolls,
        }
    }
}

#[derive(Serialize, Debug, Clone, Getters)]
pub struct BakingRightsParams {
    // chain_id, block_id, level, delegate, cycle, max_priority, has_all
    #[get = "pub(crate)"]
    chain_id: String,

    #[get = "pub(crate)"]
    block_level: i64,

    #[get = "pub(crate)"]
    block_timestamp: i64,

    #[get = "pub(crate)"]
    requested_delegate: Option<String>,

    #[get = "pub(crate)"]
    requested_cycle: Option<i64>,

    #[get = "pub(crate)"]
    requested_level: i64,
    
    #[get = "pub(crate)"]
    display_level: i64,

    #[get = "pub(crate)"]
    max_priority: i64,

    #[get = "pub(crate)"]
    has_all: bool,
}

impl BakingRightsParams {
    pub fn new(
        chain_id: String, 
        block_level: i64, 
        block_timestamp: i64,
        requested_delegate: Option<String>,
        requested_cycle: Option<i64>,
        requested_level: i64,
        display_level: i64,
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
            max_priority,
            has_all,
        }
    }
}

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
    pub fn new(
        blocks_per_cycle: i64,
        preserved_cycles: i64,
        nonce_length: i64,
        time_between_blocks: Vec<String>,
        blocks_per_roll_snapshot: i64,
        endorsers_per_block: i64
    ) -> Self {

        Self {
            blocks_per_cycle,
            preserved_cycles,
            nonce_length,
            time_between_blocks: time_between_blocks.into_iter().map(|x| x.parse().unwrap()).collect(),
            blocks_per_roll_snapshot,
            endorsers_per_block,
        }
    }
}

#[derive(Serialize, Debug, Clone, Getters)]
pub struct EndorserSlots {
    #[get = "pub(crate)"]
    contract_id: String,
    #[get = "pub(crate)"]
    slots: Vec<u8>
}

impl EndorserSlots {
    pub fn new(contract_id: String, slots: Vec<u8>) -> Self {
        Self {
            contract_id,
            slots
        }
    }
    pub fn push_to_slot(&mut self, slot: u8) {
        self.slots.push(slot);
    }
}


// cycle in which is given level
// level 0 (genesis block) is not part of any cycle (cycle 0 starts at level 1), hence the -1
pub fn cycle_from_level(level: i64, blocks_per_cycle: i64) -> i64 {
    // bad practice magic
    // FIXME
    if blocks_per_cycle == 0 {
        (level - 1) / 2048
    } else {
        (level - 1) / blocks_per_cycle
    }
}

// the position of the block in its cycle
// level 0 (genesis block) is not part of any cycle (cycle 0 starts at level 1)
// hence the blocks_per_cycle - 1 for last cycle block
pub fn level_position(level:i64, blocks_per_cycle:i64) -> i64 {
    // set defaut blocks_per_cycle in case that 0 is given as parameter
    let blocks_per_cycle = if blocks_per_cycle == 0 {
        2048
    } else {
        blocks_per_cycle
    };
    let cycle_position = (level % blocks_per_cycle) - 1;
    if cycle_position < 0 { //for last block
        blocks_per_cycle - 1
    } else {
        cycle_position
    }
}

#[derive(Debug, Fail)]
pub enum TezosPRNGError {
    #[fail(display = "Value of bound(last_roll) not correct: {} bytes", bound)]
    BoundNotCorrect {
        bound: i32
    },
}

type RandomSeedState = Vec<u8>;
pub type TezosPRNGResult = Result<(i32, RandomSeedState), TezosPRNGError>;

// tezos PRNG
// input: 
// state: RandomSeedState, initially the random seed
// nonce_size: nonce_length from current protocol constants
// blocks_per_cycle: blocks_per_cycle from current protocol constants
// use_string_bytes: string converted to bytes, i.e. endorsing rights use b"level endorsement:"
// level: block level
// offset: for baking priority, for endorsing slot
pub(crate) fn init_prng(cycle_data: &CycleData, constants: &RightsConstants, use_string_bytes: &[u8], level: i32, offset: i32) -> Result<RandomSeedState, failure::Error> {
    // a safe way to convert betwwen types is to use try_from
    let nonce_size = usize::try_from(*constants.nonce_length())?;
    let blocks_per_cycle = *constants.blocks_per_cycle();
    let state = cycle_data.random_seed(); 
    //println!("Random seed: {:?}", &state);
    // println!("Constants: {:?}", constants);
    let zero_bytes: Vec<u8> = vec![0; nonce_size];

    // the position of the block in its cycle; has to be i32
    let cycle_position: i32 = level_position(level.into(), blocks_per_cycle).try_into()?;
    // println!("[PRNG] Cycle position: {} | Requested level: {} | Offset: {}", &cycle_position, &level, &offset);

    // take the state (initially the random seed), zero bytes, the use string and the blocks position in the cycle as bytes, merge them together and hash the result
    let rd = blake2b::digest_256(&merge_slices!(&state, &zero_bytes, use_string_bytes, &cycle_position.to_be_bytes())).to_vec();
    
    // take the 4 highest bytes and xor them with the priority/slot (offset)
    let higher = num_from_slice!(rd, 0, i32) ^ offset;
    
    // set the 4 highest bytes to the result of the xor operation
    let sequence = blake2b::digest_256(&merge_slices!(&higher.to_be_bytes(), &rd[4..])).to_vec();

    Ok(sequence)
}

// tezos PRNG get number
// input: 
// state: RandomSeedState, initially the random seed
// bound: last possible roll nuber that have meaning to be generated (last_roll from context list)
// output: pseudo random generated roll number and RandomSeedState for next roll generation if the roll provided is missing from the roll list
pub fn get_pseudo_random_number(state: RandomSeedState, bound: i32) -> TezosPRNGResult {
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