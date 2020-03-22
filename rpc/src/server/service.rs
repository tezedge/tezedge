// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::{HashMap, HashSet};
use std::convert::TryInto;

use failure::{bail, format_err};
use itertools::Itertools;
use serde::{Deserialize, Serialize};

use shell::shell_channel::BlockApplied;
use shell::stats::memory::{Memory, MemoryData, MemoryStatsResult};
use storage::{BlockHeaderWithHash, BlockStorage, BlockStorageReader, ContextRecordValue, ContextStorage};
use storage::block_storage::BlockJsonData;
use storage::num_from_slice;
use storage::p2p_message_storage::P2PMessageStorage;
use storage::p2p_message_storage::rpc_message::P2PRpcMessage;
use storage::persistent::PersistentStorage;
use storage::skip_list::Bucket;
use tezos_context::channel::ContextAction;

use crate::ContextList;
use crate::encoding::context::ContextConstants;
use crate::encoding::conversions::{
    chain_id_to_string,
    contract_id_to_address,
    hash_to_contract_id,
};
use crate::helpers::{BlockHeaderInfo, ContextMap, EndorserSlots, EndorsingRight, FullBlockInfo, get_block_hash_by_block_id, get_level_by_block_id, get_prng_number, init_prng,
                     PagedResult, RightsConstants, RightsContextData, RightsParams, RpcResponseData, VoteListings};
use crate::helpers::BakingRights;
use crate::rpc_actor::RpcCollectedStateRef;
use crate::ts_to_rfc3339;
use tezos_messages::protocol::UniversalValue;

// Serialize, Deserialize,
#[derive(Serialize, Deserialize, Debug)]
pub struct Cycle
{
    #[serde(skip_serializing_if = "Option::is_none")]
    last_roll: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    nonces: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    random_seed: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    roll_snapshot: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CycleJson {
    roll_snapshot: Option<usize>,
    random_seed: Option<String>,
}

/// Retrieve blocks from database.
pub(crate) fn get_blocks(every_nth_level: Option<i32>, block_id: &str, limit: usize, persistent_storage: &PersistentStorage, state: &RpcCollectedStateRef) -> Result<Vec<FullBlockInfo>, failure::Error> {
    let block_storage = BlockStorage::new(persistent_storage);
    let block_hash = get_block_hash_by_block_id(block_id, persistent_storage, state)?;
    let blocks = match every_nth_level {
        Some(every_nth_level) => block_storage.get_every_nth_with_json_data(every_nth_level, &block_hash, limit),
        None => block_storage.get_multiple_with_json_data(&block_hash, limit),
    }?.into_iter().map(|(header, json_data)| map_header_and_json_to_full_block_info(header, json_data, state)).collect();
    Ok(blocks)
}

/// Get actions for a specific block in ascending order.
pub(crate) fn get_block_actions(block_id: &str, persistent_storage: &PersistentStorage, state: &RpcCollectedStateRef) -> Result<Vec<ContextAction>, failure::Error> {
    let context_storage = ContextStorage::new(persistent_storage);
    let block_hash = get_block_hash_by_block_id(block_id, persistent_storage, state)?;
    context_storage.get_by_block_hash(&block_hash)
        .map(|values| values.into_iter().map(|v| v.into_action()).collect())
        .map_err(|e| e.into())
}

/// Get actions for a specific contract in ascending order.
pub(crate) fn get_contract_actions(contract_id: &str, from_id: Option<u64>, limit: usize, persistent_storage: &PersistentStorage) -> Result<PagedResult<Vec<ContextRecordValue>>, failure::Error> {
    let context_storage = ContextStorage::new(persistent_storage);
    let contract_address = contract_id_to_address(contract_id)?;
    let mut context_records = context_storage.get_by_contract_address(&contract_address, from_id, limit + 1)?;
    let next_id = if context_records.len() > limit { context_records.last().map(|rec| rec.id()) } else { None };
    context_records.truncate(std::cmp::min(context_records.len(), limit));
    Ok(PagedResult::new(context_records, next_id, limit))
}

/// Return generated baking rights.
///
/// # Arguments
///
/// * `chain_id` - Url path parameter 'chain_id'.
/// * `block_id` - Url path parameter 'block_id', it contains string "head", block level or block hash.
/// * `level` - Url query parameter 'level'.
/// * `delegate` - Url query parameter 'delegate'.
/// * `cycle` - Url query parameter 'cycle'.
/// * `max_priority` - Url query parameter 'max_priority'.
/// * `has_all` - Url query parameter 'all'.
/// * `list` - Context list handler.
/// * `persistent_storage` - Persistent storage handler.
/// * `state` - Current RPC collected state (head).
///
/// Prepare all data to generate baking rights and then use Tezos PRNG to generate them.
pub(crate) fn check_and_get_baking_rights(chain_id: &str, block_id: &str, level: Option<&str>, delegate: Option<&str>, cycle: Option<&str>, max_priority: Option<&str>, has_all: bool, list: ContextList, persistent_storage: &PersistentStorage, state: &RpcCollectedStateRef) -> Result<Option<RpcResponseData>, failure::Error> {

    // get block level first
    let block_level: i64 = match get_level_by_block_id(block_id, persistent_storage, state)? {
        Some(val) => val.try_into()?,
        None => bail!("Block level not found")
    };

    let constants: RightsConstants = get_and_parse_rights_constants(&chain_id, &block_id, block_level, list.clone(), persistent_storage, state)?;

    let params: RightsParams = RightsParams::parse_rights_parameters(chain_id, level, delegate, cycle, max_priority, has_all, block_level, &constants, persistent_storage, true)?;

    let context_data: RightsContextData = RightsContextData::prepare_context_data_for_rights(params.clone(), constants.clone(), list)?;

    get_baking_rights(&context_data, &params, &constants)
}

/// Use prepared data to generate baking rights
///
/// # Arguments
///
/// * `context_data` - Data from context list used in baking and endorsing rights generation filled in [RightsContextData](RightsContextData::prepare_context_data_for_rights).
/// * `parameters` - Parameters created by [RightsParams](RightsParams::parse_rights_parameters).
/// * `constants` - Context constants used in baking and endorsing rights [`get_and_parse_rights_constants`].
#[inline]
pub(crate) fn get_baking_rights(context_data: &RightsContextData, parameters: &RightsParams, constants: &RightsConstants) -> Result<Option<RpcResponseData>, failure::Error> {
    let mut baking_rights = Vec::<BakingRights>::new();

    let blocks_per_cycle = *constants.blocks_per_cycle();
    let time_between_blocks = constants.time_between_blocks();

    let timestamp = parameters.block_timestamp();
    let block_level = parameters.block_level();

    // iterate through the whole cycle if necessery
    if let Some(cycle) = parameters.requested_cycle() {
        let first_block_level = cycle * blocks_per_cycle + 1;
        let last_block_level = first_block_level + blocks_per_cycle;

        for level in first_block_level..last_block_level {
            let seconds_to_add = (level - block_level).abs() * time_between_blocks[0];
            let estimated_timestamp = timestamp + seconds_to_add;

            // assign rolls goes here
            baking_rights_assign_rolls(&parameters, &constants, &context_data, level, estimated_timestamp, &mut baking_rights)?;

            // baking_rights = merge_slices!(&baking_rights, &level_baking_rights);
        }
    } else {
        let level = *parameters.requested_level();
        let seconds_to_add = (level - block_level).abs() * time_between_blocks[0];
        let estimated_timestamp = timestamp + seconds_to_add;
        // assign rolls goes here
        baking_rights_assign_rolls(&parameters, &constants, &context_data, level, estimated_timestamp, &mut baking_rights)?;
    }

    // if there is some delegate specified, retrive his priorities
    if let Some(delegate) = parameters.requested_delegate() {
        Ok(Some(RpcResponseData::BakingRights(baking_rights.into_iter().filter(|val| val.delegate().contains(delegate)).collect::<Vec<BakingRights>>())))
    } else {
        Ok(Some(RpcResponseData::BakingRights(baking_rights)))
    }
}

/// Use prepared data to generate baking rights
///
/// # Arguments
///
/// * `parameters` - Parameters created by [RightsParams](RightsParams::parse_rights_parameters).
/// * `constants` - Context constants used in baking and endorsing rights [`get_and_parse_rights_constants`].
/// * `context_data` - Data from context list used in baking and endorsing rights generation filled in [RightsContextData](RightsContextData::prepare_context_data_for_rights).
/// * `level` - Level to feed Tezos PRNG.
/// * `estimated_head_timestamp` - Estimated time of baking, is set to None if in past relative to block_id.
///
/// Baking priorities are are assigned to Roles, the default behavior is to include only the top priority for the delegate
#[inline]
fn baking_rights_assign_rolls(parameters: &RightsParams, constants: &RightsConstants, context_data: &RightsContextData, level: i64, estimated_head_timestamp: i64, baking_rights: &mut Vec<BakingRights>) -> Result<(), failure::Error> {
    const BAKING_USE_STRING: &[u8] = b"level baking:";

    // hashset is defined to keep track of the delegates with priorities allready assigned
    let mut assigned = HashSet::new();

    let time_between_blocks = constants.time_between_blocks();

    let max_priority = *parameters.max_priority();
    let has_all = parameters.has_all();
    let block_level = *parameters.block_level();
    let last_roll = *context_data.last_roll();
    let rolls_map = context_data.rolls();

    for priority in 0..max_priority {
        // draw the rolls for the requested parameters
        let delegate_to_assign;
        // TODO: priority can overflow in the ocaml code, do a priority % i32::max_value()
        let mut state = init_prng(&context_data, &constants, BAKING_USE_STRING, level.try_into()?, priority.try_into()?)?;

        loop {
            let (random_num, sequence) = get_prng_number(state, last_roll)?;

            if let Some(d) = rolls_map.get(&random_num) {
                delegate_to_assign = d;
                break;
            } else {
                state = sequence;
            }
        }

        // if the delegate was assgined and the the has_all flag is not set skip this priority
        if assigned.contains(&delegate_to_assign) && !has_all {
            continue;
        }

        // we omit the estimated_time field if the block on the requested level is allready baked
        if block_level < level {
            let priority_timestamp = estimated_head_timestamp + (priority as i64 * time_between_blocks[1]);
            baking_rights.push(BakingRights::new(level, delegate_to_assign.to_string(), priority.into(), Some(ts_to_rfc3339(priority_timestamp))));
        } else {
            baking_rights.push(BakingRights::new(level, delegate_to_assign.to_string(), priority.into(), None))
        }
        assigned.insert(delegate_to_assign);
    }
    Ok(())
}

/// Return generated endorsing rights.
///
/// # Arguments
///
/// * `chain_id` - Url path parameter 'chain_id'.
/// * `block_id` - Url path parameter 'block_id', it contains string "head", block level or block hash.
/// * `level` - Url query parameter 'level'.
/// * `delegate` - Url query parameter 'delegate'.
/// * `cycle` - Url query parameter 'cycle'.
/// * `has_all` - Url query parameter 'all'.
/// * `list` - Context list handler.
/// * `persistent_storage` - Persistent storage handler.
/// * `state` - Current RPC collected state (head).
///
/// Prepare all data to generate endorsing rights and then use Tezos PRNG to generate them.
pub(crate) fn check_and_get_endorsing_rights(chain_id: &str, block_id: &str, level: Option<&str>, delegate: Option<&str>, cycle: Option<&str>, has_all: bool, list: ContextList, persistent_storage: &PersistentStorage, state: &RpcCollectedStateRef) -> Result<Option<RpcResponseData>, failure::Error> {

    // get block level from block_id and from now get all nessesary data by block level
    let block_level: i64 = match get_level_by_block_id(block_id, persistent_storage, state)? {
        Some(val) => val.try_into()?,
        None => bail!("Block level not found")
    };

    let constants: RightsConstants = get_and_parse_rights_constants(chain_id, block_id, block_level, list.clone(), persistent_storage, state)?;

    let params: RightsParams = RightsParams::parse_rights_parameters(chain_id, level, delegate, cycle, None, has_all, block_level, &constants, persistent_storage, false)?;

    let context_data: RightsContextData = RightsContextData::prepare_context_data_for_rights(params.clone(), constants.clone(), list)?;

    get_endorsing_rights(&context_data, &params, &constants)
}

/// Use prepared data to generate endosring rights
///
/// # Arguments
///
/// * `context_data` - Data from context list used in baking and endorsing rights generation filled in [RightsContextData](RightsContextData::prepare_context_data_for_rights).
/// * `parameters` - Parameters created by [RightsParams](RightsParams::parse_rights_parameters).
/// * `constants` - Context constants used in baking and endorsing rights [`get_and_parse_rights_constants`].
fn get_endorsing_rights(context_data: &RightsContextData, parameters: &RightsParams, constants: &RightsConstants) -> Result<Option<RpcResponseData>, failure::Error> {

    // define helper and output variables
    let mut endorsing_rights = Vec::<EndorsingRight>::new();

    // when query param cycle is specified then iterate over all cycle levels, else only given level
    if let Some(cycle) = parameters.requested_cycle() {
        let blocks_per_cycle = *constants.blocks_per_cycle();
        let first_cycle_level = cycle * blocks_per_cycle + 1;
        let last_cycle_level = first_cycle_level + blocks_per_cycle;
        for level in first_cycle_level..last_cycle_level {
            // get estimated time first because is equal for all endorsers in given level
            // the base level for estimated time computation is level of previous block
            let estimated_time: Option<String> = parameters.get_estimated_time(constants, Some(level - 1));

            complete_endorsing_rights_for_level(context_data, parameters, constants, level, level, estimated_time, &mut endorsing_rights)?;
            // endorsing_rights = merge_slices!(&endorsing_rights, &level_endorsing_rights);
        }
    } else {
        // use level prepared during parameter parsing to compute estimated time
        let estimated_time: Option<String> = parameters.get_estimated_time(constants, None);

        complete_endorsing_rights_for_level(context_data, parameters, constants, *parameters.requested_level(), *parameters.display_level(), estimated_time, &mut endorsing_rights)?;
    };

    Ok(Some(RpcResponseData::EndorsingRights(endorsing_rights)))
}

/// Use prepared data to generate endosring rights
///
/// # Arguments
///
/// * `context_data` - Data from context list used in baking and endorsing rights generation filled in [RightsContextData](RightsContextData::prepare_context_data_for_rights).
/// * `parameters` - Parameters created by [RightsParams](RightsParams::parse_rights_parameters).
/// * `constants` - Context constants used in baking and endorsing rights [`get_and_parse_rights_constants`].
/// * `level` - Level to feed Tezos PRNG.
/// * `display_level` - Level to be displayed in output.
/// * `estimated_time` - Estimated time of endorsement, is set to None if in past relative to block_id.
#[inline]
fn complete_endorsing_rights_for_level(context_data: &RightsContextData, parameters: &RightsParams, constants: &RightsConstants, level: i64, display_level: i64, estimated_time: Option<String>, endorsing_rights: &mut Vec<EndorsingRight>) -> Result<(), failure::Error> {
    // let mut endorsing_rights = Vec::<EndorsingRight>::new();

    // endorsers_slots is needed to group all slots by delegate
    let endorsers_slots = get_endorsers_slots(constants, context_data, level)?;

    // order descending by delegate public key hash address hex byte string
    for delegate in endorsers_slots.keys().sorted().rev() {
        let delegate_data = endorsers_slots.get(delegate).ok_or(format_err!("missing EndorserSlots"))?;

        // prepare delegate contract id
        let delegate_contract_id = delegate_data.contract_id().to_string();

        // filter delegates
        if let Some(d) = parameters.requested_delegate() {
            if delegate_contract_id != d.to_string() {
                continue;
            }
        }

        endorsing_rights.push(EndorsingRight::new(
            display_level,
            delegate_contract_id,
            delegate_data.slots().clone(),
            estimated_time.clone())
        )
    }
    Ok(())
}

/// Use tezos PRNG to collect all slots for each endorser by public key hash (for later ordering of endorsers)
///
/// # Arguments
///
/// * `constants` - Context constants used in baking and endorsing rights [`get_and_parse_rights_constants`].
/// * `context_data` - Data from context list used in baking and endorsing rights generation filled in [RightsContextData](RightsContextData::prepare_context_data_for_rights).
/// * `level` - Level to feed Tezos PRNG.
#[inline]
fn get_endorsers_slots(constants: &RightsConstants, context_data: &RightsContextData, level: i64) -> Result<HashMap<String, EndorserSlots>, failure::Error> {
    // special byte string used in Tezos PRNG
    const ENDORSEMENT_USE_STRING: &[u8] = b"level endorsement:";
    // prepare helper variable
    let mut endorsers_slots: HashMap<String, EndorserSlots> = HashMap::new();

    for endorser_slot in (0..*constants.endorsers_per_block() as u8).rev() {
        // generate PRNG per endorsement slot and take delegates by roll number from context_rolls
        // if roll number is not found then reroll with new state till roll nuber is found in context_rolls
        let mut state = init_prng(&context_data, &constants, ENDORSEMENT_USE_STRING, level.try_into()?, endorser_slot.try_into()?)?;
        loop {
            let (random_num, sequence) = get_prng_number(state, *context_data.last_roll())?;

            if let Some(delegate) = context_data.rolls().get(&random_num) {
                // collect all slots for each delegate
                // convert contract id to public key hash address hex byte string (needed for later ordering)
                let public_key_hash = hex::encode(contract_id_to_address(&delegate)?);
                let endorsers_slots_entry = endorsers_slots.entry(public_key_hash).or_insert(EndorserSlots::new(delegate.clone(), Vec::new()));
                endorsers_slots_entry.push_to_slot(endorser_slot);
                break;
            } else {
                state = sequence;
            }
        }
    }
    Ok(endorsers_slots)
}

/// Get information about current head
pub(crate) fn get_full_current_head(state: &RpcCollectedStateRef) -> Result<Option<FullBlockInfo>, failure::Error> {
    let state = state.read().unwrap();
    let current_head = state.current_head().as_ref().map(|current_head| {
        let chain_id = chain_id_to_string(state.chain_id());
        FullBlockInfo::new(current_head, &chain_id)
    });

    Ok(current_head)
}

/// Get information about current head header
pub(crate) fn get_current_head_header(state: &RpcCollectedStateRef) -> Result<Option<BlockHeaderInfo>, failure::Error> {
    let state = state.read().unwrap();
    let current_head = state.current_head().as_ref().map(|current_head| {
        let chain_id = chain_id_to_string(state.chain_id());
        BlockHeaderInfo::new(current_head, &chain_id)
    });

    Ok(current_head)
}

/// Get information about block
pub(crate) fn get_full_block(block_id: &str, persistent_storage: &PersistentStorage, state: &RpcCollectedStateRef) -> Result<Option<FullBlockInfo>, failure::Error> {
    let block_storage = BlockStorage::new(persistent_storage);
    let block;
    // hotfix
    // TODO: rework block_id to accept types String and integer for block levels
    match block_id.parse() {
        Ok(val) => {
            block = block_storage.get_by_block_level_with_json_data(val)?.map(|(header, json_data)| map_header_and_json_to_full_block_info(header, json_data, &state));
        }
        Err(_e) => {
            let block_hash = get_block_hash_by_block_id(block_id, persistent_storage, state)?;
            block = block_storage.get_with_json_data(&block_hash)?.map(|(header, json_data)| map_header_and_json_to_full_block_info(header, json_data, &state));
        }
    }
    Ok(block)
}

/// Get information about block header
pub(crate) fn get_block_header(block_id: &str, persistent_storage: &PersistentStorage, state: &RpcCollectedStateRef) -> Result<Option<BlockHeaderInfo>, failure::Error> {
    let block_storage = BlockStorage::new(persistent_storage);
    let block_hash = get_block_hash_by_block_id(block_id, persistent_storage, state)?;
    let block = block_storage.get_with_json_data(&block_hash)?.map(|(header, json_data)| map_header_and_json_to_block_header_info(header, json_data, state));

    Ok(block)
}

/// Get protocol context constants from context list
///
/// # Arguments
///
/// * `chain_id` - id of chain, not used
/// * `block_id` - Url path parameter 'block_id', it contains string "head", block level or block hash.
/// * `opt_level` - Optionaly input block level from block_id if is already known to prevent double code execution.
/// * `list` - Context list handler.
/// * `persistent_storage` - Persistent storage handler.
/// * `state` - Current RPC collected state (head).
pub(crate) fn get_context_constants(_chain_id: &str, block_id: &str, opt_level: Option<i64>, list: ContextList, persistent_storage: &PersistentStorage, state: &RpcCollectedStateRef) -> Result<Option<ContextConstants>, failure::Error> {
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
            panic!(format!("Protocol not found in block: {}", block_id))
        }

        if let Some(Bucket::Exists(data)) = reader.get_key(level, &"data/v1/constants".to_string())? {
            constants = data;
        } else {
            constants = Default::default();
        }
    };

    Ok(Some(ContextConstants::transpile_context_bytes(&constants, protocol_hash)?))
}

/// Get protocol context constants from context list
/// (just for RPC render use-case, do not use in processing or algorithms)
///
/// # Arguments
///
/// * `block_id` - Url path parameter 'block_id', it contains string "head", block level or block hash.
/// * `opt_level` - Optionaly input block level from block_id if is already known to prevent double code execution.
/// * `list` - Context list handler.
/// * `persistent_storage` - Persistent storage handler.
/// * `state` - Current RPC collected state (head).
pub(crate) fn get_context_constants_just_for_rpc(
    block_id: &str,
    opt_level: Option<i64>,
    list: ContextList,
    persistent_storage: &PersistentStorage,
    state: &RpcCollectedStateRef) -> Result<Option<HashMap<&'static str, UniversalValue>>, failure::Error> {

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
            panic!(format!("Protocol not found in block: {}, level: {}", block_id, level))
        }

        if let Some(Bucket::Exists(data)) = reader.get_key(level, &"data/v1/constants".to_string())? {
            constants = data;
        } else {
            constants = Default::default();
        }
    };

    Ok(tezos_messages::protocol::get_constants_for_rpc(&constants, protocol_hash)?)
}

pub(crate) fn get_cycle_from_context(level: &str, list: ContextList) -> Result<Option<HashMap<String, Cycle>>, failure::Error> {
    let ctxt_level: usize = level.parse().unwrap();

    let context_data = {
        let reader = list.read().expect("mutex poisoning");
        if let Ok(Some(c)) = reader.get(ctxt_level) {
            c
        } else {
            bail!("Context data not found")
        }
    };

    // get cylce list from context storage
    let cycle_lists: HashMap<String, Bucket<Vec<u8>>> = context_data.clone().into_iter()
        .filter(|(k, _)| k.contains("cycle"))
        .filter(|(_, v)| match v {
            Bucket::Exists(_) => true,
            _ => false
        })
        .collect();

    // transform cycle list
    let mut cycles: HashMap<String, Cycle> = HashMap::new();

    // process every key value pair
    for (key, bucket) in cycle_lists.iter() {

        // create vector from path
        let path: Vec<&str> = key.split('/').collect();

        // convert value from bytes to hex
        let value = match bucket {
            Bucket::Exists(value) => hex::encode(value).to_string(),
            _ => "".to_string()
        };

        // create new cycle
        // TODO: !!! check path and move to match
        cycles.entry(path[2].to_string()).or_insert(Cycle {
            last_roll: None,
            nonces: None,
            random_seed: None,
            roll_snapshot: None,
        });

        // process cycle key value pairs
        match path.as_slice() {
            ["data", "cycle", cycle, "random_seed"] => {
                // println!("cycle: {:?} random_seed: {:?}", cycle, value );
                cycles.entry(cycle.to_string()).and_modify(|cycle| {
                    cycle.random_seed = Some(value);
                });
            }
            ["data", "cycle", cycle, "roll_snapshot"] => {
                // println!("cycle: {:?} roll_snapshot: {:?}", cycle, value);
                cycles.entry(cycle.to_string()).and_modify(|cycle| {
                    cycle.roll_snapshot = Some(value);
                });
            }
            ["data", "cycle", cycle, "nonces", nonces] => {
                // println!("cycle: {:?} nonces: {:?}/{:?}", cycle, nonces, value)
                cycles.entry(cycle.to_string()).and_modify(|cycle| {
                    match cycle.nonces.as_mut() {
                        Some(entry) => entry.insert(nonces.to_string(), value),
                        None => {
                            cycle.nonces = Some(HashMap::new());
                            cycle.nonces.as_mut().unwrap().insert(nonces.to_string(), value)
                        }
                    };
                });
            }
            ["data", "cycle", cycle, "last_roll", last_roll] => {
                // println!("cycle: {:?} last_roll: {:?}/{:?}", cycle, last_roll, value)
                cycles.entry(cycle.to_string()).and_modify(|cycle| {
                    match cycle.last_roll.as_mut() {
                        Some(entry) => entry.insert(last_roll.to_string(), value),
                        None => {
                            cycle.last_roll = Some(HashMap::new());
                            cycle.last_roll.as_mut().unwrap().insert(last_roll.to_string(), value)
                        }
                    };
                });
            }
            _ => ()
        };
    }

    Ok(Some(cycles))
}

pub(crate) fn get_cycle_from_context_as_json(level: &str, cycle_id: &str, list: ContextList) -> Result<Option<CycleJson>, failure::Error> {
    let level: usize = level.parse()?;

    let list = list.read().expect("mutex poisoning");
    let random_seed = list.get_key(level, &format!("data/cycle/{}/random_seed", &cycle_id));
    let roll_snapshot = list.get_key(level, &format!("data/cycle/{}/roll_snapshot", &cycle_id));
    match (random_seed, roll_snapshot) {
        (Ok(Some(random_seed)), Ok(Some(roll_snapshot))) => {
            let cycle_json = CycleJson {
                random_seed: if let Bucket::Exists(value) = random_seed { Some(hex::encode(value).to_string()) } else { None },
                roll_snapshot: if let Bucket::Exists(value) = roll_snapshot { hex::encode(value).parse().ok() } else { None },
            };
            Ok(Some(cycle_json))
        }
        _ => bail!("Context data not found")
    }
}

pub(crate) fn get_rolls_owner_current_from_context(level: &str, list: ContextList) -> Result<Option<HashMap<String, HashMap<String, HashMap<String, String>>>>, failure::Error> {
    let ctxt_level: usize = level.parse().unwrap();
    // println!("level: {:?}", ctxt_level);

    let context_data = {
        let reader = list.read().expect("mutex poisoning");
        if let Ok(Some(c)) = reader.get(ctxt_level) {
            c
        } else {
            bail!("Context data not found")
        }
    };

    // get rolls list from context storage
    let rolls_lists: HashMap<String, Bucket<Vec<u8>>> = context_data.clone().into_iter()
        .filter(|(k, _)| k.contains("rolls/owner/current"))
        .filter(|(_, v)| match v {
            Bucket::Exists(_) => true,
            _ => false
        })
        .collect();

    // create rolls list
    let mut rolls: HashMap<String, HashMap<String, HashMap<String, String>>> = HashMap::new();

    // process every key value pair
    for (key, bucket) in rolls_lists.iter() {

        // create vector from path
        let path: Vec<&str> = key.split('/').collect();

        // convert value from bytes to hex
        let value = match bucket {
            Bucket::Exists(value) => hex::encode(value).to_string(),
            _ => "".to_string()
        };

        // process roll key value pairs
        match path.as_slice() {
            ["data", "rolls", "owner", "current", path1, path2, path3 ] => {
                // println!("rolls: {:?}/{:?}/{:?} value: {:?}", path1, path2, path3, value );

                rolls.entry(path1.to_string())
                    .and_modify(|roll| {
                        roll.entry(path2.to_string())
                            .and_modify(|index2| {
                                index2.insert(path3.to_string(), value.to_string());
                            })
                            .or_insert({
                                let mut index3 = HashMap::new();
                                index3.insert(path3.to_string(), value.to_string());
                                index3
                            });
                    })
                    .or_insert({
                        let mut index2 = HashMap::new();
                        index2.entry(path2.to_string())
                            .or_insert({
                                let mut index3 = HashMap::new();
                                index3.insert(path3.to_string(), value.to_string());
                                index3
                            });
                        index2
                    });
            }
            _ => ()
        }
    }


    Ok(Some(rolls))
}

pub(crate) fn retrieve_p2p_messages(start: &str, count: &str, persistent_storage: &PersistentStorage) -> Result<Vec<P2PRpcMessage>, failure::Error> {
    let p2p_store = P2PMessageStorage::new(persistent_storage);
    let start = start.parse().unwrap();
    let count = count.parse().unwrap();
    if let Ok(data) = p2p_store.get_range(start, count) {
        Ok(data)
    } else {
        Ok(Default::default())
    }
}

pub(crate) fn retrieve_host_p2p_messages(start: &str, end: &str, host: &str, persistent_storage: &PersistentStorage) -> Result<Vec<P2PRpcMessage>, failure::Error> {
    let p2p_store = P2PMessageStorage::new(persistent_storage);
    let start = start.parse().unwrap();
    let end = end.parse().unwrap();
    let host = host.parse().unwrap();
    Ok(p2p_store.get_range_for_host(host, start, end)?)
}

pub(crate) fn get_votes_listings(_chain_id: &str, block_id: &str, persistent_storage: &PersistentStorage, context_list: ContextList, state: &RpcCollectedStateRef) -> Result<Option<Vec<VoteListings>>, failure::Error> {
    let mut listings = Vec::<VoteListings>::new();

    // get block level first
    let block_level: i64 = match get_level_by_block_id(block_id, persistent_storage, state)? {
        Some(val) => val.try_into()?,
        None => bail!("Block level not found")
    };

    // get the whole context
    let ctxt = get_context(&block_level.to_string(), context_list)?;

    // filter out the listings data
    let listings_data: ContextMap = ctxt.unwrap().into_iter()
        .filter(|(k, _)| k.starts_with(&"data/votes/listings/"))
        .collect();

    // convert the raw context data to VoteListings
    for (key, value) in listings_data.into_iter() {
        if let Bucket::Exists(data) = value {
            // get the address an the curve tag from the key (e.g. data/votes/listings/ed25519/2c/ca/28/ab/01/9ae2d8c26f4ce4924cad67a2dc6618)
            let address = key.split("/").skip(4).take(6).join("");
            let curve = key.split("/").skip(3).take(1).join("");

            let address_decoded = hash_to_contract_id(&address, &curve)?;
            listings.push(VoteListings::new(address_decoded, num_from_slice!(data, 0, i32)));
        }
    }

    // sort the vector in reverse ordering (as in ocaml node)
    listings.sort();
    listings.reverse();
    Ok(Some(listings))
}

pub(crate) fn get_stats_memory() -> MemoryStatsResult<MemoryData> {
    let memory = Memory::new();
    memory.get_memory_stats()
}

pub(crate) fn get_context(level: &str, list: ContextList) -> Result<Option<HashMap<String, Bucket<Vec<u8>>>>, failure::Error> {
    let level = level.parse()?;
    {
        let storage = list.read().expect("poisoned storage lock");
        storage.get(level).map_err(|e| e.into())
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
fn get_and_parse_rights_constants(chain_id: &str, block_id: &str, block_level: i64, list: ContextList, persistent_storage: &PersistentStorage, state: &RpcCollectedStateRef) -> Result<RightsConstants, failure::Error> {
    let constants = match get_context_constants(chain_id, block_id, Some(block_level), list.clone(), persistent_storage, state)? {
        Some(v) => v,
        None => bail!("Cannot get protocol constants")
    };

    Ok(RightsConstants::new(
        *constants.blocks_per_cycle(),
        *constants.preserved_cycles(),
        *constants.nonce_length(),
        constants.time_between_blocks().to_vec().into_iter().map(|x| x.parse().unwrap()).collect(),
        *constants.blocks_per_roll_snapshot(),
        *constants.endorsers_per_block(),
    ))
}

#[inline]
fn map_header_and_json_to_full_block_info(header: BlockHeaderWithHash, json_data: BlockJsonData, state: &RpcCollectedStateRef) -> FullBlockInfo {
    let state = state.read().unwrap();
    let chain_id = chain_id_to_string(state.chain_id());
    FullBlockInfo::new(&BlockApplied::new(header, json_data), &chain_id)
}

#[inline]
fn map_header_and_json_to_block_header_info(header: BlockHeaderWithHash, json_data: BlockJsonData, state: &RpcCollectedStateRef) -> BlockHeaderInfo {
    let state = state.read().unwrap();
    let chain_id = chain_id_to_string(state.chain_id());
    BlockHeaderInfo::new(&BlockApplied::new(header, json_data), &chain_id)
}


