// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use slog::Logger;

use crypto::hash::BlockHash;
use shell::stats::memory::{Memory, MemoryData, MemoryStatsResult};
use storage::context::actions::context_action_storage::{
    contract_id_to_contract_address_for_index, ContextActionBlockDetails, ContextActionFilters,
    ContextActionJson, ContextActionRecordValue, ContextActionStorageReader, ContextActionType,
};
use storage::context::merkle::merkle_storage_stats::MerkleStoragePerfReport;
use storage::context::{ContextApi, TezedgeContext};
use storage::PersistentStorage;
use tezos_context::channel::ContextAction;
use tezos_messages::base::rpc_support::UniversalValue;

use crate::helpers::PagedResult;
use crate::server::RpcServiceEnvironment;
use crate::services::protocol::get_context_protocol_params;

/// Get actions for a specific block in ascending order.
#[allow(dead_code)]
pub(crate) fn get_block_actions(
    block_hash: BlockHash,
    persistent_storage: &PersistentStorage,
) -> Result<Vec<ContextAction>, failure::Error> {
    get_block_actions_by_hash(
        &ensure_context_action_storage(persistent_storage)?,
        &block_hash,
    )
}

#[allow(dead_code)]
pub(crate) fn get_block_actions_by_hash(
    context_action_storage: &ContextActionStorageReader,
    block_hash: &BlockHash,
) -> Result<Vec<ContextAction>, failure::Error> {
    context_action_storage
        .get_by_block_hash(&block_hash)
        .map(|values| values.into_iter().map(|v| v.into_action()).collect())
        .map_err(|e| e.into())
}

pub(crate) fn get_block_actions_cursor(
    block_hash: BlockHash,
    cursor_id: Option<u64>,
    limit: Option<usize>,
    action_types: Option<&str>,
    persistent_storage: &PersistentStorage,
) -> Result<Vec<ContextActionJson>, failure::Error> {
    let context_action_storage = ensure_context_action_storage(persistent_storage)?;
    let mut filters = ContextActionFilters::with_block_hash(block_hash.into());
    if let Some(action_types) = action_types {
        filters = filters.with_action_types(get_action_types(action_types));
    }
    let values = context_action_storage
        .load_cursor(cursor_id, limit, filters)?
        .into_iter()
        .map(ContextActionJson::from)
        .collect();
    Ok(values)
}

pub(crate) fn get_block_action_details(
    block_hash: BlockHash,
    persistent_storage: &PersistentStorage,
) -> Result<ContextActionBlockDetails, failure::Error> {
    let context_action_storage = ensure_context_action_storage(persistent_storage)?;

    let actions: Vec<ContextAction> = context_action_storage
        .get_by_block_hash(&block_hash)?
        .into_iter()
        .map(|action_record| action_record.action)
        .collect();

    Ok(ContextActionBlockDetails::calculate_block_action_details(
        actions,
    ))
}

pub(crate) fn get_contract_actions_cursor(
    contract_address: &str,
    cursor_id: Option<u64>,
    limit: Option<usize>,
    action_types: Option<&str>,
    persistent_storage: &PersistentStorage,
) -> Result<Vec<ContextActionJson>, failure::Error> {
    let context_action_storage = ensure_context_action_storage(persistent_storage)?;
    let contract_address = contract_id_to_contract_address_for_index(contract_address)?;
    let mut filters = ContextActionFilters::with_contract_id(contract_address);
    if let Some(action_types) = action_types {
        filters = filters.with_action_types(get_action_types(action_types));
    }
    let values = context_action_storage
        .load_cursor(cursor_id, limit, filters)?
        .into_iter()
        .map(ContextActionJson::from)
        .collect();
    Ok(values)
}

/// Get actions for a specific contract in ascending order.
#[allow(dead_code)]
pub(crate) fn get_contract_actions(
    contract_id: &str,
    from_id: Option<u64>,
    limit: usize,
    persistent_storage: &PersistentStorage,
) -> Result<PagedResult<Vec<ContextActionRecordValue>>, failure::Error> {
    let context_action_storage = ensure_context_action_storage(persistent_storage)?;
    let contract_address = contract_id_to_contract_address_for_index(contract_id)?;
    let mut context_records =
        context_action_storage.get_by_contract_address(&contract_address, from_id, limit + 1)?;
    let next_id = if context_records.len() > limit {
        context_records.last().map(|rec| rec.id())
    } else {
        None
    };
    context_records.truncate(std::cmp::min(context_records.len(), limit));
    Ok(PagedResult::new(context_records, next_id, limit))
}

pub(crate) fn get_stats_memory() -> MemoryStatsResult<MemoryData> {
    let memory = Memory::new();
    memory.get_memory_stats()
}

pub(crate) fn get_stats_memory_protocol_runners() -> MemoryStatsResult<Vec<MemoryData>> {
    let memory = Memory::new();
    memory.get_memory_stats_protocol_runners()
}

pub(crate) fn get_context_stats(
    context: &TezedgeContext,
) -> Result<MerkleStoragePerfReport, failure::Error> {
    Ok(context.get_merkle_stats()?)
}

pub(crate) fn get_cycle_length_for_block(
    block_hash: &BlockHash,
    env: &RpcServiceEnvironment,
    log: &Logger,
) -> Result<i32, failure::Error> {
    if let Ok(context_proto_params) = get_context_protocol_params(block_hash, env) {
        Ok(tezos_messages::protocol::get_constants_for_rpc(
            &context_proto_params.constants_data,
            &context_proto_params.protocol_hash,
        )?
            .map(|constants| constants.get("blocks_per_cycle")
                .map(|value| if let UniversalValue::Number(value) = value { *value } else {
                    slog::warn!(log, "Cycle length missing"; "block" => block_hash.to_base58_check());
                    4096
                })
            ).flatten().unwrap_or_else(|| {
            slog::warn!(log, "Cycle length missing"; "block" => block_hash.to_base58_check());
            4096
        }))
    } else {
        slog::warn!(log, "Cycle length missing"; "block" => block_hash.to_base58_check());
        Ok(4096)
    }
}

pub(crate) fn get_dev_version() -> String {
    let version_env: &'static str = env!("CARGO_PKG_VERSION");

    format!("v{}", version_env.to_string())
}

#[inline]
pub(crate) fn get_action_types(action_types: &str) -> Vec<ContextActionType> {
    action_types
        .split(',')
        .filter_map(|x: &str| x.parse().ok())
        .collect()
}

#[inline]
pub(crate) fn ensure_context_action_storage(
    persistent_storage: &PersistentStorage,
) -> Result<ContextActionStorageReader, failure::Error> {
    match persistent_storage.merkle_context_actions() {
        None => Err(failure::format_err!(
            "Persistent context actions storage is not initialized!"
        )),
        Some(context_action_storage) => Ok(ContextActionStorageReader::new(context_action_storage)),
    }
}
