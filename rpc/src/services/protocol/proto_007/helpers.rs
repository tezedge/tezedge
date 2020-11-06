// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;
use std::convert::TryFrom;
use std::convert::TryInto;

use failure::{bail, Fail, format_err};
use getset::Getters;

use crypto::blake2b;
use storage::{context_key, num_from_slice};
use storage::context::{ContextApi, TezedgeContext};
use storage::context_action_storage::contract_id_to_contract_address_for_index;
use storage::persistent::PersistentStorage;
use tezos_messages::base::signature_public_key_hash::SignaturePublicKeyHash;
use tezos_messages::p2p::binary_message::BinaryMessage;

use crate::helpers::{ContextProtocolParam, get_block_timestamp_by_level};
use crate::merge_slices;

/// Context constants used in baking and endorsing rights
#[derive(Debug, Clone, Getters)]
pub struct RightsConstants {
    #[get = "pub(crate)"]
    blocks_per_cycle: i32,
    #[get = "pub(crate)"]
    preserved_cycles: u8,
    #[get = "pub(crate)"]
    nonce_length: u8,
    #[get = "pub(crate)"]
    time_between_blocks: Vec<i64>,
    #[get = "pub(crate)"]
    blocks_per_roll_snapshot: i32,
    #[get = "pub(crate)"]
    endorsers_per_block: u16,
}

impl RightsConstants {
    /// simple constructor to create RightsConstants
    pub fn new(
        blocks_per_cycle: i32,
        preserved_cycles: u8,
        nonce_length: u8,
        time_between_blocks: Vec<i64>,
        blocks_per_roll_snapshot: i32,
        endorsers_per_block: u16,
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

    /// Get all context constants which are used in endorsing and baking rights generation
    ///
    /// # Arguments
    ///
    /// * `chain_id` - Url path parameter 'chain_id'.
    /// * `block_id` - Url path parameter 'block_id', it contains string "head", block level or block hash.
    /// * `block_level` - Provide level of block_id that is already known to prevent double code execution.
    /// * `persistent_storage` - Persistent storage handler.
    /// * `state` - Current RPC collected state (head).
    #[inline]
    pub(crate) fn parse_rights_constants(context_proto_param: ContextProtocolParam) -> Result<Self, failure::Error> {
        let dynamic = tezos_messages::protocol::proto_007::constants::ParametricConstants::from_bytes(context_proto_param.constants_data)?;
        let fixed = tezos_messages::protocol::proto_007::constants::FIXED;

        Ok(RightsConstants::new(
            dynamic.blocks_per_cycle(),
            dynamic.preserved_cycles(),
            fixed.nonce_length(),
            dynamic.time_between_blocks().clone(),
            dynamic.blocks_per_roll_snapshot(),
            dynamic.endorsers_per_block(),
        ))
    }
}

/// Data from context DB for completing baking and endorsing rights
#[derive(Debug, Clone, Getters)]
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
    pub(crate) fn prepare_context_data_for_rights(parameters: RightsParams, constants: RightsConstants, context: TezedgeContext) -> Result<Self, failure::Error> {
        // prepare constants that are used
        let blocks_per_cycle = *constants.blocks_per_cycle();

        // prepare parameters that are used
        let block_level = *parameters.block_level();
        let requested_level = *parameters.requested_level();

        // prepare cycle for which rollers are selected
        let requested_cycle = if let Some(cycle) = *parameters.requested_cycle() {
            cycle
        } else {
            cycle_from_level(requested_level, blocks_per_cycle)?
        };

        // get context_hash from level
        let ctx_hash = context.level_to_hash(block_level.try_into()?)?;

        // get index of roll snapshot
        let roll_snapshot: i16 = {
            if let Some(data) = context.get_key_from_history(&ctx_hash, &context_key!("data/cycle/{}/roll_snapshot", requested_cycle))? {
                num_from_slice!(data, 0, i16)
            } else { // key not found - prepare error for later processing
                return Err(format_err!("roll_snapshot"));
            }
        };

        let random_seed = {
            if let Some(data) = context.get_key_from_history(&ctx_hash, &context_key!("data/cycle/{}/random_seed", requested_cycle))? {
                data
            } else { // key not found - prepare error for later processing
                return Err(format_err!("random_seed"));
            }
        };

        // Snapshots of last_roll are listed from 0 same as roll_snapshot.
        let last_roll = {
            if let Some(data) = context.get_key_from_history(&ctx_hash, &context_key!("data/cycle/{}/last_roll/{}", requested_cycle, roll_snapshot))? {
                num_from_slice!(data, 0, i32)
            } else { // key not found - prepare error for later processing
                return Err(format_err!("last_roll"));
            }
        };

        // get list of rolls from context list
        let context_rolls = if let Some(rolls) = Self::get_context_rolls(&context, block_level, requested_cycle, roll_snapshot)? {
            rolls
        } else {
            return Err(format_err!("rolls"));
        };

        Ok(Self::new(
            random_seed.to_vec(),
            last_roll,
            context_rolls,
        ))
    }

    /// get list of rollers from context list selected by snapshot level
    ///
    /// # Arguments
    ///
    /// * `context` - context list HashMap from [get_context_as_hashmap](RightsContextData::get_context_as_hashmap)
    ///
    /// Return rollers for [RightsContextData.rolls](RightsContextData.rolls)
    fn get_context_rolls(context: &TezedgeContext, requested_level: i64, cycle: i64, snapshot: i16) -> Result<Option<HashMap<i32, String>>, failure::Error> {

        // get context_hash from level
        let ctx_hash = context.level_to_hash(requested_level.try_into()?)?;

        let rolls = if let Some(val) = context.get_key_values_by_prefix(&ctx_hash, &context_key!("data/rolls/owner/snapshot/{}/{}", cycle, snapshot))? {
            val
        } else {
            bail!("No rolls found in context")
        };

        let mut roll_owners: HashMap<i32, String> = HashMap::new();

        // iterate through all the owners,the roll_num is the last component of the key, decode the value (it is a public key) to get the public key hash address (tz1...)
        for (key, value) in rolls.into_iter() {
            if key.last().is_none() {
                continue;
            }
            let roll_num = key.last().unwrap();

            // the values are public keys
            let delegate = SignaturePublicKeyHash::from_tagged_bytes(value.clone())?.to_string();
            roll_owners.insert(roll_num.parse()?, delegate);
        }
        if roll_owners.len() == 0 {
            bail!("No rolls assigned, all rolls happened to be DELETED")
        }
        Ok(Some(roll_owners))
    }
}

/// Set of parameters used to complete baking and endorsing rights
#[derive(Debug, Clone, Getters)]
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

    /// Level to be displayed in output.
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
        is_baking_rights: bool,
    ) -> Result<Self, failure::Error> {
        let preserved_cycles = *rights_constants.preserved_cycles();
        let blocks_per_cycle = *rights_constants.blocks_per_cycle();

        // this is the cycle of block_id level
        let current_cycle = cycle_from_level(block_level, blocks_per_cycle)?;

        // display_level is here because of corner case where all levels < 1 are computed as level 1 but oputputed as they are
        let mut display_level: i64 = block_level;
        // endorsing rights only: timestamp_level is the base level for timestamp computation is taken from last known block (block_id) timestamp + time_between_blocks[0]
        let mut timestamp_level: i64 = block_level;
        // Check the param_level, if there is a level specified validate it, if no set it to the block_level
        // requested_level is used to get data from context list
        let requested_level: i64 = match param_level {
            Some(level) => {
                let level = level.parse()?;
                // check the bounds for the requested level (if it is in the previous/next preserved cycles)
                Self::validate_cycle(cycle_from_level(level, blocks_per_cycle)?, current_cycle, preserved_cycles)?;
                // display level is always same as level requested
                display_level = level;
                // endorsing rights: to compute timestamp for level parameter there need to be taken timestamp of previous block
                timestamp_level = level - 1;
                // there can be requested also negative level, this is not an error but it is handled same as level 1 would be requested
                // In Ocaml Tezos node there can be negative level provided as query parameter, it is not handled as error but it will instead return endorsing/baking rights for level 1
                // for all reuqested negative levels use level 1
                if level < 1 {
                    1
                } else {
                    level
                }
            }
            // here is the main difference between endorsing and baking rights which data are loaded from context list based on level
            None => if is_baking_rights {
                display_level = block_level + 1;
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
            param_has_all,
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
    pub fn get_estimated_time(&self, constants: &RightsConstants, level: Option<i64>) -> Option<i64> {
        // if is cycle then level is provided as parameter else use prepared timestamp_level
        let timestamp_level = level.unwrap_or(self.timestamp_level);

        //check if estimated time is computed and convert from raw epoch time to rfc3339 format
        if self.block_level <= timestamp_level {
            let est_timestamp = ((timestamp_level - self.block_level).abs() as i64 * constants.time_between_blocks()[0]) + self.block_timestamp;
            Some(est_timestamp)
        } else {
            None
        }
    }

    /// Validate if cycle requested as url query parameter (cycle or level) is available in context list by checking preserved_cycles constant
    #[inline]
    fn validate_cycle(requested_cycle: i64, current_cycle: i64, preserved_cycles: u8) -> Result<i64, failure::Error> {
        if (requested_cycle - current_cycle).abs() <= (preserved_cycles as i64) {
            Ok(requested_cycle)
        } else {
            bail!("Requested cycle out of bounds") //TODO: prepare cycle error
        }
    }
}

/// Struct used in endorsing rights to map endorsers to endorsement slots before they can be ordered and completed
#[derive(Debug, Clone, Getters)]
pub struct EndorserSlots {
    /// endorser contract id in form:tz1.../tz2.../KT1...
    #[get = "pub(crate)"]
    contract_id: String,

    /// Orderer vector of endorsement slots
    #[get = "pub(crate)"]
    slots: Vec<u16>,
}

impl EndorserSlots {
    /// Simple constructor that return EndorserSlots
    pub fn new(contract_id: String, slots: Vec<u16>) -> Self {
        Self {
            contract_id,
            slots,
        }
    }

    /// Push endorsing slot to slots
    pub fn push_to_slot(&mut self, slot: u16) {
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
pub fn cycle_from_level(level: i64, blocks_per_cycle: i32) -> Result<i64, failure::Error> {
    // check if blocks_per_cycle is not 0 to prevent panic
    if blocks_per_cycle > 0 {
        Ok((level - 1) / (blocks_per_cycle as i64))
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
pub fn level_position(level: i32, blocks_per_cycle: i32) -> Result<i32, failure::Error> {
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
    let cycle_position: i32 = level_position(level, blocks_per_cycle)?;

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
        return Err(TezosPRNGError::BoundNotCorrect { bound: bound });
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

#[inline]
pub fn create_index_from_contract_id(contract_id: &str) -> Result<Vec<String>, failure::Error> {
    const INDEX_SIZE: usize = 6;
    let mut index = Vec::new();

    // input validation is handled by the contract_id_to_address function 
    let address = contract_id_to_contract_address_for_index(contract_id)?;

    let hashed = hex::encode(blake2b::digest_256(&address));

    for elem in (0..INDEX_SIZE * 2).step_by(2) {
        index.push(hashed[elem..elem + 2].to_string());
    }

    Ok(index)
}

/// Returns something like this: "data/contracts/index/91/6e/d7/72/4e/49/0000535110affdb82923710d1ec205f26ba8820a2259"
pub(crate) fn construct_indexed_contract_key(pkh: &str) -> Result<String, failure::Error> {
    const KEY_PREFIX: &str = "data/contracts/index";
    let index = create_index_from_contract_id(pkh)?.join("/");
    let key = hex::encode(contract_id_to_contract_address_for_index(pkh)?);

    Ok(format!("{}/{}/{}", KEY_PREFIX, index, key))
}
