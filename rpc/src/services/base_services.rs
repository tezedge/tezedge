// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::convert::TryInto;

use failure::bail;
use slog::Logger;

use crypto::hash::{chain_id_to_b58_string, HashType};
use shell::shell_channel::BlockApplied;
use shell::stats::memory::{Memory, MemoryData, MemoryStatsResult};
use storage::{BlockHeaderWithHash, BlockStorage, BlockStorageReader, ContextActionRecordValue, ContextActionStorage};
use storage::block_storage::BlockJsonData;
use storage::context::{ContextApi, TezedgeContext};
use storage::context_action_storage::{ContextActionFilters, ContextActionJson, contract_id_to_contract_address_for_index};
use storage::merkle_storage::{MerkleStorageStats, StringTree};
use storage::persistent::PersistentStorage;
use tezos_context::channel::ContextAction;
use tezos_messages::p2p::encoding::version::NetworkVersion;
use tezos_messages::protocol::{RpcJsonMap, UniversalValue};

use crate::helpers::{BlockHeaderInfo, BlockHeaderShellInfo, FullBlockInfo, get_action_types, get_block_hash_by_block_id, get_context_protocol_params, get_level_by_block_id, MonitorHeadStream, NodeVersion, PagedResult, Protocols};
use crate::rpc_actor::RpcCollectedStateRef;
use crate::server::RpcServiceEnvironment;

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

/// Get information about current head shell header
pub(crate) fn get_current_head_shell_header(state: &RpcCollectedStateRef) -> Result<Option<BlockHeaderShellInfo>, failure::Error> {
    let state = state.read().unwrap();
    let current_head = state.current_head().as_ref().map(|current_head| {
        let chain_id = chain_id_to_b58_string(state.chain_id());
        BlockHeaderInfo::new(current_head, &chain_id).to_shell_header()
    });

    Ok(current_head)
}

/// Get information about current head monitor header as a stream of Json strings
pub(crate) fn get_current_head_monitor_header(state: &RpcCollectedStateRef) -> Result<Option<MonitorHeadStream>, failure::Error> {

    // create and return the a new stream on rpc call 
    Ok(Some(MonitorHeadStream {
        state: state.clone(),
        last_polled_timestamp: None,
    }))
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
    let block_hash = HashType::BlockHash.string_to_bytes(&get_block_hash(block_id, persistent_storage, state)?)?;
    let block = block_storage.get_with_json_data(&block_hash)?.map(|(header, json_data)| map_header_and_json_to_block_header_info(header, json_data, state).to_shell_header());

    Ok(block)
}

pub(crate) fn live_blocks(_chain_param: &str, block_param: &str, env: &RpcServiceEnvironment) -> Result<Option<Vec<String>>, failure::Error> {
    let persistent_storage = env.persistent_storage();
    let state = env.state();

    let block_storage = BlockStorage::new(persistent_storage);
    let block_hash = get_block_hash_by_block_id(block_param, persistent_storage, state)?;
    let current_block = block_storage.get_with_additional_data(&block_hash)?;
    let max_ttl: usize = match current_block {
        Some((_, json_data)) => {
            json_data.max_operations_ttl().into()
        }
        None => bail!("Block not found for block id: {}", block_param)
    };
    let block_level = get_block_level_by_block_id(block_param, 0, persistent_storage, state)?;

    let live_blocks: Option<Vec<String>> = block_storage.get_live_blocks(block_level.try_into()?, max_ttl)?
        .map(|blocks| blocks.iter().map(|b| HashType::BlockHash.bytes_to_string(&b)).collect());

    Ok(live_blocks)
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
    persistent_storage: &PersistentStorage,
    state: &RpcCollectedStateRef) -> Result<Option<RpcJsonMap>, failure::Error> {
    let context_proto_params = get_context_protocol_params(
        block_id,
        opt_level,
        persistent_storage,
        state,
    )?;

    Ok(tezos_messages::protocol::get_constants_for_rpc(&context_proto_params.constants_data, context_proto_params.protocol_hash)?)
}

pub(crate) fn get_cycle_length_for_block(block_id: &str, storage: &PersistentStorage, state: &RpcCollectedStateRef, log: &Logger) -> Result<i32, failure::Error> {
    if let Ok(context_proto_params) = get_context_protocol_params(block_id, None, storage, state) {
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

pub(crate) fn get_context_raw_bytes(block_id: &str, prefix: Option<&str>, persistent_storage: &PersistentStorage, context: &TezedgeContext, state: &RpcCollectedStateRef, log: &Logger) -> Result<Option<StringTree>, failure::Error> {
    // TODO: should be replaced by context_hash
    // get block level first
    let ctxt_level: i32 = match get_level_by_block_id(block_id, persistent_storage, state) {
        Ok(Some(val)) => {
            let rv = val.try_into();
            if rv.is_err() {
                slog::warn!(log, "Block level not found");
                return Ok(None);
            }
            rv.unwrap()
        }
        _ => {
            slog::warn!(log, "Block level not found");
            return Ok(None);
        }
    };

    let ctx_hash = context.level_to_hash(ctxt_level);
    if ctx_hash.is_err() {
        slog::warn!(log, "Block level not found");
        return Ok(None);
    }
    match context.get_context_tree_by_prefix(&ctx_hash.unwrap(), &prefix) {
        Ok(tree) => Ok(Some(tree)),
        Err(_) => Ok(None), // return None to avoid a panic
    }
}

pub(crate) fn get_stats_memory() -> MemoryStatsResult<MemoryData> {
    let memory = Memory::new();
    memory.get_memory_stats()
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

pub(crate) fn get_node_version(network_version: &NetworkVersion) -> Result<NodeVersion, failure::Error> {
    Ok(NodeVersion::new(network_version))
}

pub(crate) fn get_database_memstats(context: &TezedgeContext) -> Result<MerkleStorageStats, failure::Error> {
    let stats = context.get_merkle_stats()?;

    Ok(stats)
}

pub(crate) fn get_block_by_block_id(block_id: &str, persistent_storage: &PersistentStorage, state: &RpcCollectedStateRef) -> Result<Option<FullBlockInfo>, failure::Error> {
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
pub(crate) fn get_block_level_by_block_id(block_id: &str, offset: i64, persistent_storage: &PersistentStorage, state: &RpcCollectedStateRef) -> Result<i64, failure::Error> {
    if block_id == "genesis" {
        // genesis alias is allways for the block on level 0
        Ok(0)
    } else if block_id == "head" {
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


