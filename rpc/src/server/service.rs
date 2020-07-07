// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::{HashMap};
use std::convert::TryInto;

use failure::bail;
use serde::{Deserialize, Serialize};

use crypto::hash::{chain_id_to_b58_string};
use shell::shell_channel::BlockApplied;
use shell::stats::memory::{Memory, MemoryData, MemoryStatsResult};
use storage::{BlockHeaderWithHash, BlockStorage, BlockStorageReader, ContextActionRecordValue, ContextActionStorage, num_from_slice};
use storage::block_storage::BlockJsonData;
use storage::persistent::{PersistentStorage, ContextMap};
use storage::skip_list::Bucket;
use storage::context::{TezedgeContext, ContextIndex, ContextApi};
use tezos_context::channel::ContextAction;
use tezos_messages::protocol::{RpcJsonMap, UniversalValue};

use crate::ContextList;
use crate::helpers::{BlockHeaderInfo, BlockHeaderShellInfo, FullBlockInfo, get_block_hash_by_block_id, get_context_protocol_params, PagedResult, get_action_types, Protocols};
use crate::rpc_actor::RpcCollectedStateRef;
use storage::context_action_storage::{contract_id_to_contract_address_for_index, ContextActionFilters, ContextActionJson};
use slog::Logger;

// Serialize, Deserialize,
#[derive(Serialize, Deserialize, Debug)]
pub struct Cycle {
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
    roll_snapshot: Option<i16>,
    random_seed: Option<String>,
}

pub type BlockOperations = Vec<String>;

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
#[allow(dead_code)]
pub(crate) fn get_block_actions(block_id: &str, persistent_storage: &PersistentStorage, state: &RpcCollectedStateRef) -> Result<Vec<ContextAction>, failure::Error> {
    let context_action_storage = ContextActionStorage::new(persistent_storage);
    let block_hash = get_block_hash_by_block_id(block_id, persistent_storage, state)?;
    get_block_actions_by_hash(&context_action_storage, &block_hash)
}

pub(crate) fn get_block_actions_by_hash(context_action_storage: &ContextActionStorage, block_hash: &Vec<u8>) -> Result<Vec<ContextAction>, failure::Error> {
    context_action_storage.get_by_block_hash(&block_hash)
        .map(|values| values.into_iter().map(|v| v.into_action()).collect())
        .map_err(|e| e.into())
}

pub(crate) fn get_block_actions_cursor(block_id: &str, cursor_id: Option<u64>, limit: Option<usize>, action_types: Option<&str>, persistent_storage: &PersistentStorage, state: &RpcCollectedStateRef) -> Result<Vec<ContextActionJson>, failure::Error> {
    let context_action_storage = ContextActionStorage::new(persistent_storage);
    let block_hash = get_block_hash_by_block_id(block_id, persistent_storage, state)?;
    let mut filters = ContextActionFilters::with_block_hash(block_hash);
    if let Some(action_types) = action_types {
        filters = filters.with_action_types(get_action_types(action_types));
    }
    let values = context_action_storage.load_cursor(cursor_id, limit, filters)?
        .into_iter().map(|value| ContextActionJson::from(value))
        .collect();
    Ok(values)
}

pub(crate) fn get_contract_actions_cursor(contract_address: &str, cursor_id: Option<u64>, limit: Option<usize>, action_types: Option<&str>, persistent_storage: &PersistentStorage) -> Result<Vec<ContextActionJson>, failure::Error> {
    let context_action_storage = ContextActionStorage::new(persistent_storage);
    let contract_address = contract_id_to_contract_address_for_index(contract_address)?;
    let mut filters = ContextActionFilters::with_contract_id(contract_address);
    if let Some(action_types) = action_types {
        filters = filters.with_action_types(get_action_types(action_types));
    }
    let values = context_action_storage.load_cursor(cursor_id, limit, filters)?
        .into_iter().map(|value| ContextActionJson::from(value))
        .collect();
    Ok(values)
}

/// Get actions for a specific contract in ascending order.
#[allow(dead_code)]
pub(crate) fn get_contract_actions(contract_id: &str, from_id: Option<u64>, limit: usize, persistent_storage: &PersistentStorage) -> Result<PagedResult<Vec<ContextActionRecordValue>>, failure::Error> {
    let context_action_storage = ContextActionStorage::new(persistent_storage);
    let contract_address = contract_id_to_contract_address_for_index(contract_id)?;
    let mut context_records = context_action_storage.get_by_contract_address(&contract_address, from_id, limit + 1)?;
    let next_id = if context_records.len() > limit { context_records.last().map(|rec| rec.id()) } else { None };
    context_records.truncate(std::cmp::min(context_records.len(), limit));
    Ok(PagedResult::new(context_records, next_id, limit))
}

/// Get information about current head
pub(crate) fn get_full_current_head(state: &RpcCollectedStateRef) -> Result<Option<FullBlockInfo>, failure::Error> {
    let state = state.read().unwrap();
    let current_head = state.current_head().as_ref().map(|current_head| {
        let chain_id = chain_id_to_b58_string(state.chain_id());
        FullBlockInfo::new(current_head, &chain_id)
    });

    Ok(current_head)
}

/// Get information about current head header
pub(crate) fn get_current_head_header(state: &RpcCollectedStateRef) -> Result<Option<BlockHeaderInfo>, failure::Error> {
    let state = state.read().unwrap();
    let current_head = state.current_head().as_ref().map(|current_head| {
        let chain_id = chain_id_to_b58_string(state.chain_id());
        BlockHeaderInfo::new(current_head, &chain_id)
    });

    Ok(current_head)
}

/// Get information about current head header
pub(crate) fn get_current_head_shell_header(state: &RpcCollectedStateRef) -> Result<Option<BlockHeaderShellInfo>, failure::Error> {
    let state = state.read().unwrap();
    let current_head = state.current_head().as_ref().map(|current_head| {
        let chain_id = chain_id_to_b58_string(state.chain_id());
        BlockHeaderInfo::new(current_head, &chain_id).to_shell_header()
    });

    Ok(current_head)
}

/// Get information about block
pub(crate) fn get_full_block(block_id: &str, persistent_storage: &PersistentStorage, state: &RpcCollectedStateRef) -> Result<Option<FullBlockInfo>, failure::Error> {
    let block = get_block_by_block_id(block_id, persistent_storage, state)?;
    Ok(block)
}

/// Get information about block header
pub(crate) fn get_block_header(block_id: &str, persistent_storage: &PersistentStorage, state: &RpcCollectedStateRef) -> Result<Option<BlockHeaderInfo>, failure::Error> {
    let block_storage = BlockStorage::new(persistent_storage);
    let block_hash = get_block_hash_by_block_id(block_id, persistent_storage, state)?;
    let block = block_storage.get_with_json_data(&block_hash)?.map(|(header, json_data)| map_header_and_json_to_block_header_info(header, json_data, state));

    Ok(block)
}

/// Get information about block shell header
pub(crate) fn get_block_shell_header(block_id: &str, persistent_storage: &PersistentStorage, state: &RpcCollectedStateRef) -> Result<Option<BlockHeaderShellInfo>, failure::Error> {
    let block_storage = BlockStorage::new(persistent_storage);
    let block_hash = get_block_hash_by_block_id(block_id, persistent_storage, state)?;
    let block = block_storage.get_with_json_data(&block_hash)?.map(|(header, json_data)| map_header_and_json_to_block_header_info(header, json_data, state).to_shell_header());

    Ok(block)
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
    state: &RpcCollectedStateRef) -> Result<Option<RpcJsonMap>, failure::Error> {
    let context_proto_params = get_context_protocol_params(
        block_id,
        opt_level,
        list,
        persistent_storage,
        state,
    )?;

    Ok(tezos_messages::protocol::get_constants_for_rpc(&context_proto_params.constants_data, context_proto_params.protocol_hash)?)
}

pub(crate) fn get_cycle_length_for_block(block_id: &str, list: ContextList, storage: &PersistentStorage, state: &RpcCollectedStateRef, log: &Logger) -> Result<i32, failure::Error> {
    if let Ok(context_proto_params) = get_context_protocol_params(block_id, None, list, storage, state) {
        Ok(tezos_messages::protocol::get_constants_for_rpc(&context_proto_params.constants_data, context_proto_params.protocol_hash)?
            .map(|constants| constants.get("blocks_per_cycle")
                .map(|value| if let UniversalValue::Number(value) = value { *value } else {
                    slog::warn!(log, "Cycle length missing"; "block" => block_id);
                    4096
                })
            ).flatten().unwrap_or_else(|| {
            slog::warn!(log, "Cycle length missing"; "block" => block_id);
            4096
        }))
    } else {
        slog::warn!(log, "Cycle length missing"; "block" => block_id);
        Ok(4096)
    }
}

pub(crate) fn get_cycle_from_context(level: &str, list: ContextList, persistent_storage: &PersistentStorage) -> Result<Option<HashMap<String, Cycle>>, failure::Error> {
    let ctxt_level: usize = level.parse().unwrap();

    // let context_data = {
    //     let reader = list.read().expect("mutex poisoning");
    //     if let Ok(Some(c)) = reader.get(ctxt_level) {
    //         c
    //     } else {
    //         bail!("Context data not found")
    //     }
    // };

    let context = TezedgeContext::new(BlockStorage::new(&persistent_storage), list.clone());
    let context_index = ContextIndex::new(Some(ctxt_level.try_into()?), None);
    let context_data = context.get_by_key_prefix(&context_index, &vec!["data/cycle/".to_string()])?;

    // get cylce list from context storage
    // let cycle_lists: HashMap<String, Bucket<Vec<u8>>> = context_data.clone().into_iter()
    //     .filter(|(k, _)| k.contains("cycle"))
    //     .filter(|(_, v)| match v {
    //         Bucket::Exists(_) => true,
    //         _ => false
    //     })
    //     .collect();
    let cycle_lists = if let Some(ctx) = context_data {
        ctx
    } else {
        bail!("Error getting context data")
    };

    // transform cycle list
    let mut cycles: HashMap<String, Cycle> = HashMap::new();

    // process every key value pair
    for (key, bucket) in cycle_lists.iter() {

        // create vector from path
        let path: Vec<&str> = key.split('/').collect();

        // convert value from bytes to hex
        let value = match bucket {
            Bucket::Exists(value) => hex::encode(value).to_string(),
            _ => continue
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
                cycles.entry(cycle.to_string()).and_modify(|cycle| {
                    cycle.random_seed = Some(value);
                });
            }
            ["data", "cycle", cycle, "roll_snapshot"] => {
                cycles.entry(cycle.to_string()).and_modify(|cycle| {
                    cycle.roll_snapshot = Some(value);
                });
            }
            ["data", "cycle", cycle, "nonces", nonces] => {
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

pub(crate) fn get_cycle_from_context_as_json(level: &str, cycle_id: &str, list: ContextList, persistent_storage: &PersistentStorage) -> Result<Option<CycleJson>, failure::Error> {
    let level: usize = level.parse()?;

    let context = TezedgeContext::new(BlockStorage::new(&persistent_storage), list.clone());
    let context_index = ContextIndex::new(Some(level.try_into()?), None);

    let random_seed = context.get_key(&context_index, &vec![format!("data/cycle/{}/random_seed", &cycle_id)])?; // list.get_key(level, &format!("data/cycle/{}/random_seed", &cycle_id));
    let roll_snapshot = context.get_key(&context_index, &vec![format!("data/cycle/{}/roll_snapshot", &cycle_id)])?;

    match (random_seed, roll_snapshot) {
        (Some(random_seed), Some(roll_snapshot)) => {
            let cycle_json = CycleJson {
                random_seed: if let Bucket::Exists(value) = random_seed { Some(hex::encode(value).to_string()) } else { None },
                roll_snapshot: if let Bucket::Exists(value) = roll_snapshot { Some(num_from_slice!(value, 0, i16)) } else { None },
            };
            Ok(Some(cycle_json))
        }
        _ => bail!("Context data not found")
    }
}

pub(crate) fn get_rolls_owner_current_from_context(level: &str, list: ContextList, persistent_storage: &PersistentStorage) -> Result<Option<HashMap<String, HashMap<String, HashMap<String, String>>>>, failure::Error> {
    let ctxt_level: usize = level.parse().unwrap();

    let context = TezedgeContext::new(BlockStorage::new(&persistent_storage), list.clone());
    let context_index = ContextIndex::new(Some(ctxt_level.try_into()?), None);
    let context_data = context.get_by_key_prefix(&context_index, &vec!["data/rolls/owner/current/".to_string()])?;

    // create rolls list
    let mut rolls: HashMap<String, HashMap<String, HashMap<String, String>>> = HashMap::new();

    let rolls_lists = if let Some(ctx) = context_data {
        ctx
    } else {
        bail!("Error getting context data")
    };
    // process every key value pair
    for (key, bucket) in rolls_lists.iter() {

        // create vector from path
        let path: Vec<&str> = key.split('/').collect();

        // convert value from bytes to hex
        let value = match bucket {
            Bucket::Exists(value) => hex::encode(value).to_string(),
            _ => continue
        };

        // process roll key value pairs
        match path.as_slice() {
            ["data", "rolls", "owner", "current", path1, path2, path3 ] => {

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

pub(crate) fn get_stats_memory() -> MemoryStatsResult<MemoryData> {
    let memory = Memory::new();
    memory.get_memory_stats()
}

pub(crate) fn get_context(level: &str, list: ContextList) -> Result<Option<ContextMap>, failure::Error> {
    crate::helpers::get_context(level, list)
}

/// Extract the current_protocol and the next_protocol from the block metadata
pub(crate) fn get_block_protocols(block_id: &str, persistent_storage: &PersistentStorage, state: &RpcCollectedStateRef) -> Result<Protocols, failure::Error> {
    let block = get_block_by_block_id(block_id, persistent_storage, state)?;

    if let Some(block_info) = block {
        Ok(Protocols::new(
            block_info.metadata["protocol"].to_string().replace("\"", ""),
            block_info.metadata["next_protocol"].to_string().replace("\"", ""),
        ))
    } else {
        bail!("Cannot retrieve protocols, block_id {} not found!", block_id)
    }
    
}

/// Extract the hash from the block data
pub(crate) fn get_block_hash(block_id: &str, persistent_storage: &PersistentStorage, state: &RpcCollectedStateRef) -> Result<String, failure::Error> {
    let block = get_block_by_block_id(block_id, persistent_storage, state)?;

    if let Some(block_info) = block {
        Ok(block_info.hash)
    } else {
        bail!("Cannot retrieve block hash, block_id {} not found!", block_id)
    }
    
}

/// Returns the chain id for the requested chain
pub(crate) fn get_chain_id(state: &RpcCollectedStateRef) -> Result<String, failure::Error> {
    // Note: for now, we only support one chain
    // TODO: rework to support multiple chains
    let state = state.read().unwrap();
    Ok(chain_id_to_b58_string(state.chain_id()))
}

/// Returns the chain id for the requested chain
pub(crate) fn get_block_operation_hashes(block_id: &str, persistent_storage: &PersistentStorage, state: &RpcCollectedStateRef) -> Result<Vec<BlockOperations>, failure::Error> {
    let block = get_block_by_block_id(block_id, persistent_storage, state)?;

    if let Some(block_info) = block {
        let operations = block_info.operations.into_iter()
            .map(|op_group| op_group.into_iter()
                .map(|op| op["hash"].to_string().replace("\"", ""))
                .collect())
            .collect();
        Ok(operations)
    } else {
        bail!("Cannot retrieve operation hashes from block, block_id {} not found!", block_id)
    }
}

pub(crate) fn get_block_by_block_id(block_id: &str, persistent_storage: &PersistentStorage, state: &RpcCollectedStateRef) -> Result<Option<FullBlockInfo>, failure::Error>{
    let block_storage = BlockStorage::new(persistent_storage);

    // check whether block_id is formated like: head~10 (block 10 levels in the past from head)
    let offseted_id: Vec<&str> = block_id.split("~").collect();
    
    // get the level of the requested block_id (offset if necessery)
    let level = if offseted_id.len() == 2 {
        match offseted_id[1].parse::<i64>() {
            Ok(offset) => {
                get_block_level_by_block_id(offseted_id[0], offset, persistent_storage, state)?
            }
            Err(_) => bail!("Offset parameter should be a number! Offset parameter: {}", offseted_id[1])
        }
    } else {
        get_block_level_by_block_id(block_id, 0, persistent_storage, state)?
    };

    // get the block by level (or level with offset)
    Ok(block_storage.get_by_block_level_with_json_data(level.try_into()?)?.map(|(header, json_data)| map_header_and_json_to_full_block_info(header, json_data, &state)))
}

/// Extract block level from the full block info
fn get_block_level_by_block_id(block_id: &str, offset: i64, persistent_storage: &PersistentStorage, state: &RpcCollectedStateRef) -> Result<i64, failure::Error> {
    if block_id == "head" {
        match get_full_current_head(state)? {
            Some(head) => {
                Ok(head.header.level as i64 - offset)
            }
            None => bail!("Head not yet initialized")
        }
    } else {
        match block_id.parse::<i64>() {
            Ok(val) => {
                Ok(val - offset)
            }
            Err(_) => {
                let block_storage = BlockStorage::new(persistent_storage);
                let block_hash = get_block_hash_by_block_id(block_id, persistent_storage, state)?;
                let block = block_storage.get_with_json_data(&block_hash)?.map(|(header, json_data)| map_header_and_json_to_full_block_info(header, json_data, &state));
                if let Some(block) = block {
                    Ok(block.header.level as i64 - offset)
                } else {
                    bail!("Cannot get block level from block {}, block not found", block_id)
                }
            }
        }
    }
}

#[inline]
fn map_header_and_json_to_full_block_info(header: BlockHeaderWithHash, json_data: BlockJsonData, state: &RpcCollectedStateRef) -> FullBlockInfo {
    let state = state.read().unwrap();
    let chain_id = chain_id_to_b58_string(state.chain_id());
    FullBlockInfo::new(&BlockApplied::new(header, json_data), &chain_id)
}

#[inline]
fn map_header_and_json_to_block_header_info(header: BlockHeaderWithHash, json_data: BlockJsonData, state: &RpcCollectedStateRef) -> BlockHeaderInfo {
    let state = state.read().unwrap();
    let chain_id = chain_id_to_b58_string(state.chain_id());
    BlockHeaderInfo::new(&BlockApplied::new(header, json_data), &chain_id)
}


