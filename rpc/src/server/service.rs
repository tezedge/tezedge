// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::{HashMap};

use failure::bail;
use serde::{Deserialize, Serialize};

use crypto::hash::{chain_id_to_b58_string};
use shell::shell_channel::BlockApplied;
use shell::stats::memory::{Memory, MemoryData, MemoryStatsResult};
use storage::{BlockHeaderWithHash, BlockStorage, BlockStorageReader, ContextActionRecordValue, ContextActionStorage};
use storage::block_storage::BlockJsonData;
use storage::p2p_message_storage::P2PMessageStorage;
use storage::p2p_message_storage::rpc_message::P2PRpcMessage;
use storage::persistent::PersistentStorage;
use storage::skip_list::Bucket;
use tezos_context::channel::ContextAction;
use tezos_messages::protocol::RpcJsonMap;

use crate::ContextList;
use crate::helpers::{BlockHeaderInfo, FullBlockInfo, get_block_hash_by_block_id, get_context_protocol_params, PagedResult};
use crate::rpc_actor::RpcCollectedStateRef;
use storage::context_action_storage::contract_id_to_contract_address_for_index;

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
    let context_action_storage = ContextActionStorage::new(persistent_storage);
    let block_hash = get_block_hash_by_block_id(block_id, persistent_storage, state)?;
    get_block_actions_by_hash(&context_action_storage, &block_hash)
}

pub(crate) fn get_block_actions_by_hash(context_action_storage: &ContextActionStorage, block_hash: &Vec<u8>) -> Result<Vec<ContextAction>, failure::Error> {
    context_action_storage.get_by_block_hash(&block_hash)
        .map(|values| values.into_iter().map(|v| v.into_action()).collect())
        .map_err(|e| e.into())
}

/// Get actions for a specific contract in ascending order.
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

pub(crate) fn get_stats_memory() -> MemoryStatsResult<MemoryData> {
    let memory = Memory::new();
    memory.get_memory_stats()
}

pub(crate) fn get_context(level: &str, list: ContextList) -> Result<Option<HashMap<String, Bucket<Vec<u8>>>>, failure::Error> {
    crate::helpers::get_context(level, list)
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


