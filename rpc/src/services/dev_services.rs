// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

// TODO - TE-261: many things commented out here because they don't work with the new
// context until we reintroduce something equivalent to the context actions storage.
// The timings database, along with the readonly IPC context access could be used
// to reproduce the same functionality.

use crypto::hash::{BlockPayloadHash, ContractKt1Hash, OperationHash};
use serde::{Deserialize, Serialize};
use shell_automaton::mempool::{OperationKind, OperationValidationResult};
use shell_automaton::service::rpc_service::{BakingState, RpcShellAutomatonActionsRaw};
use shell_automaton::service::statistics_service::ActionKindStatsForBlock;
use shell_automaton::{Action, ActionWithMeta};
use slog::Logger;
use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::convert::TryFrom;
use std::vec;
use storage::shell_automaton_action_meta_storage::ShellAutomatonActionStatsForRanges;

use crypto::hash::{BlockHash, ChainId, ContractTz1Hash, ContractTz2Hash, ContractTz3Hash};
use shell::stats::memory::{Memory, MemoryData, MemoryStatsResult};
use shell_automaton::service::rpc_service::RpcRequest as RpcShellAutomatonMsg;
use shell_automaton::ActionId;
use storage::cycle_eras_storage::CycleEra;
use storage::database::backend::BoxedSliceKV;
use storage::database::error::Error as DBError;
use storage::persistent::Decoder;
//use tezos_context::actions::context_action_storage::{
//    contract_id_to_contract_address_for_index, ContextActionBlockDetails, ContextActionFilters,
//    ContextActionJson, ContextActionRecordValue, ContextActionStorageReader, ContextActionType,
//};
use storage::{
    BlockMetaStorage, BlockMetaStorageReader, BlockStorage, BlockStorageReader, ConstantsStorage,
    CycleErasStorage, Direction, IteratorMode, PersistentStorage, ShellAutomatonActionStorage,
    ShellAutomatonStateStorage, StorageError,
};
//use tezos_context::channel::ContextAction;
use tezos_messages::base::ConversionError;
use tezos_messages::p2p::encoding::block_header::Level;

use crate::helpers::{BlockMetadata, PagedResult, RpcServiceError};
use crate::server::RpcServiceEnvironment;

use crate::services::protocol::get_blocks_per_cycle;

use super::base_services::{get_additional_data_or_fail, get_raw_block_header_with_hash};

pub type ContractAddress = Vec<u8>;

/// Get actions for a specific block in ascending order.
#[allow(dead_code)]
pub(crate) fn get_block_actions(
    block_hash: BlockHash,
    persistent_storage: &PersistentStorage,
) -> Result<Vec<() /*ContextAction*/>, RpcServiceError> {
    get_block_actions_by_hash(
        &ensure_context_action_storage(persistent_storage)?,
        &block_hash,
    )
}

#[allow(dead_code)]
pub(crate) fn get_block_actions_by_hash(
    _context_action_storage: &(), /*&ContextActionStorageReader,*/
    _block_hash: &BlockHash,
) -> Result<Vec<() /*ContextAction*/>, RpcServiceError> {
    //context_action_storage
    //    .get_by_block_hash(&block_hash)
    //    .map(|values| values.into_iter().map(|v| v.into_action()).collect())
    //    .map_err(|e| e.into())
    Err(RpcServiceError::UnexpectedError {
        reason: "Persistent context actions storage is not implemented!".to_string(),
    })
}

pub(crate) fn get_block_actions_cursor(
    _block_hash: BlockHash,
    _cursor_id: Option<u64>,
    _limit: Option<usize>,
    _action_types: Option<&str>,
    _persistent_storage: &PersistentStorage,
) -> Result<Vec<() /*ContextActionJson*/>, RpcServiceError> {
    //let context_action_storage = ensure_context_action_storage(persistent_storage)?;
    //let mut filters = ContextActionFilters::with_block_hash(block_hash.into());
    //if let Some(action_types) = action_types {
    //    filters = filters.with_action_types(get_action_types(action_types));
    //}
    //let values = context_action_storage
    //    .load_cursor(cursor_id, limit, filters)?
    //    .into_iter()
    //    .map(ContextActionJson::from)
    //    .collect();
    //Ok(values)
    Err(RpcServiceError::UnexpectedError {
        reason: "Persistent context actions storage is not implemented!".to_string(),
    })
}

pub(crate) fn get_block_action_details(
    _block_hash: BlockHash,
    _persistent_storage: &PersistentStorage,
) -> Result<() /*ContextActionBlockDetails*/, RpcServiceError> {
    //let context_action_storage = ensure_context_action_storage(persistent_storage)?;

    //let actions: Vec<ContextAction> = context_action_storage
    //    .get_by_block_hash(&block_hash)?
    //    .into_iter()
    //    .map(|action_record| action_record.action)
    //    .collect();

    //Ok(ContextActionBlockDetails::calculate_block_action_details(
    //    actions,
    //))
    Err(RpcServiceError::UnexpectedError {
        reason: "Persistent context actions storage is not implemented!".to_string(),
    })
}

pub(crate) fn get_contract_actions_cursor(
    _contract_address: &str,
    _cursor_id: Option<u64>,
    _limit: Option<usize>,
    _action_types: Option<&str>,
    _persistent_storage: &PersistentStorage,
) -> Result<Vec<() /*ContextActionJson*/>, RpcServiceError> {
    //let context_action_storage = ensure_context_action_storage(persistent_storage)?;
    //let contract_address = contract_id_to_contract_address_for_index(contract_address)?;
    //let mut filters = ContextActionFilters::with_contract_id(contract_address);
    //if let Some(action_types) = action_types {
    //    filters = filters.with_action_types(get_action_types(action_types));
    //}
    //let values = context_action_storage
    //    .load_cursor(cursor_id, limit, filters)?
    //    .into_iter()
    //    .map(ContextActionJson::from)
    //    .collect();
    //Ok(values)
    Err(RpcServiceError::UnexpectedError {
        reason: "Persistent context actions storage is not implemented!".to_string(),
    })
}

/// Get actions for a specific contract in ascending order.
#[allow(dead_code)]
pub(crate) fn get_contract_actions(
    _contract_id: &str,
    _from_id: Option<u64>,
    _limit: usize,
    _persistent_storage: &PersistentStorage,
) -> Result<PagedResult<Vec<() /*ContextActionRecordValue*/>>, RpcServiceError> {
    //let context_action_storage = ensure_context_action_storage(persistent_storage)?;
    //let contract_address = contract_id_to_contract_address_for_index(contract_id)?;
    //let mut context_records =
    //    context_action_storage.get_by_contract_address(&contract_address, from_id, limit + 1)?;
    //let next_id = if context_records.len() > limit {
    //    context_records.last().map(|rec| rec.id())
    //} else {
    //    None
    //};
    //context_records.truncate(std::cmp::min(context_records.len(), limit));
    //Ok(PagedResult::new(context_records, next_id, limit))
    Err(RpcServiceError::UnexpectedError {
        reason: "Persistent context actions storage is not implemented!".to_string(),
    })
}

pub(crate) fn get_stats_memory() -> MemoryStatsResult<MemoryData> {
    let memory = Memory::new();
    memory.get_memory_stats()
}

pub(crate) fn get_stats_memory_protocol_runners() -> MemoryStatsResult<Vec<MemoryData>> {
    let memory = Memory::new();
    memory.get_memory_stats_protocol_runners()
}

pub(crate) fn get_cycle_length_for_block(
    chain_id: &ChainId,
    block_hash: &BlockHash,
    env: &RpcServiceEnvironment,
    _: &Logger,
) -> Result<i32, RpcServiceError> {
    // get the protocol hash
    let protocol_hash =
        &get_additional_data_or_fail(chain_id, block_hash, env.persistent_storage())?.protocol_hash;

    let block_level =
        get_raw_block_header_with_hash(chain_id, block_hash, env.persistent_storage())?
            .header
            .level();

    // proto 10 and beyond
    if let Some(eras) = CycleErasStorage::new(env.persistent_storage()).get(protocol_hash)? {
        for era in eras {
            if *era.first_level() > block_level {
                continue;
            } else {
                return Ok(*era.blocks_per_cycle());
            }
        }
        Err(RpcServiceError::NoDataFoundError {
            reason: "No matching cycle era found".into(),
        })
    } else {
        // if no eras are present, simply get blocks_per_cycle from constatns (proto 001-009)
        if let Some(constants) =
            ConstantsStorage::new(env.persistent_storage()).get(protocol_hash)?
        {
            match get_blocks_per_cycle(protocol_hash, &constants) {
                Ok(blocks_per_cycle) => Ok(blocks_per_cycle),
                Err(e) => Err(RpcServiceError::NoDataFoundError {
                    reason: e.to_string(),
                }),
            }
        } else {
            Err(RpcServiceError::NoDataFoundError {
                reason: "No constants found for protocol".into(),
            })
        }
    }
}

pub(crate) fn get_cycle_eras(
    chain_id: &ChainId,
    block_hash: &BlockHash,
    env: &RpcServiceEnvironment,
    _: &Logger,
) -> Result<Vec<CycleEra>, RpcServiceError> {
    let protocol_hash =
        &get_additional_data_or_fail(chain_id, block_hash, env.persistent_storage())?.protocol_hash;

    if let Some(eras) = CycleErasStorage::new(env.persistent_storage()).get(protocol_hash)? {
        Ok(eras)
    } else {
        Ok(vec![])
    }
}

pub(crate) fn get_dev_version() -> String {
    let version_env: &'static str = env!("CARGO_PKG_VERSION");

    format!("v{}", version_env)
}

#[inline]
pub(crate) fn _get_action_types(_action_types: &str) -> Vec<() /*ContextActionType*/> {
    //action_types
    //    .split(',')
    //    .filter_map(|x: &str| x.parse().ok())
    //    .collect()
    vec![]
}

#[inline]
pub(crate) fn ensure_context_action_storage(
    _persistent_storage: &PersistentStorage,
) -> Result<() /*ContextActionStorageReader*/, RpcServiceError> {
    Err(RpcServiceError::UnexpectedError {
        reason: "Persistent context actions storage is not implemented!".to_string(),
    })
}

/// Retrieve blocks from database.
pub(crate) async fn get_blocks(
    _chain_id: ChainId,
    block_hash: BlockHash,
    every_nth_level: Option<i32>,
    limit: usize,
    env: &RpcServiceEnvironment,
) -> Result<Vec<SlimBlockData>, RpcServiceError> {
    let block_meta_storage = BlockMetaStorage::new(env.persistent_storage());

    let blocks = match every_nth_level {
        Some(every_nth_level) => BlockStorage::new(env.persistent_storage())
            .get_every_nth_with_json_data(every_nth_level, &block_hash, limit),
        None => BlockStorage::new(env.persistent_storage())
            .get_multiple_with_json_data(&block_hash, limit),
    }?;

    // NOTE: using a single connection here, but could connect to multiple runners, worth it?
    let mut connection = env.tezos_protocol_api().readable_connection().await?;

    let mut result = Vec::with_capacity(blocks.len());

    for (block_header, block_json_data) in blocks {
        if let Some(block_additional_data) = block_meta_storage.get_additional_data(&block_hash)? {
            let response = connection
                .apply_block_result_metadata(
                    block_header.header.context().clone(),
                    block_json_data.block_header_proto_metadata_bytes,
                    block_additional_data.max_operations_ttl().into(),
                    block_additional_data.protocol_hash,
                    block_additional_data.next_protocol_hash,
                )
                .await;

            let response = if let Ok(response) = response {
                response
            } else {
                continue;
            };

            let metadata: BlockMetadata = serde_json::from_str(&response).unwrap_or_default();
            let cycle_position = if let Some(level) = metadata.get("level") {
                level["cycle_position"].as_i64()
            } else if let Some(level) = metadata.get("level_info") {
                level["cycle_position"].as_i64()
            } else {
                None
            };

            result.push(SlimBlockData {
                level: block_header.header.level(),
                block_hash: block_header.hash.to_base58_check(),
                timestamp: block_header.header.timestamp().to_string(),
                cycle_position,
            });
        }
    }
    Ok(result)
}

/// Struct to show in tezedge explorer to lower data flow
#[derive(Serialize, Debug, Clone)]
pub struct SlimBlockData {
    pub level: i32,
    pub block_hash: String,
    pub timestamp: String,
    // TODO: TE-199 Refactor FullBlockInfo (should be i32)
    // Note: serde's Value can be converted into Option<i64> without panicing, the original tezos value is an i32
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cycle_position: Option<i64>,
}

/// Dedicated function to convert contract id to contract address for indexing in storage action,
/// contract id index has specified length [LEN_TOTAL]
///
/// # Arguments
///
/// * `contract_id` - contract id (tz... or KT1...)
#[inline]
pub(crate) fn contract_id_to_contract_address_for_index(
    contract_id: &str,
) -> Result<ContractAddress, ConversionError> {
    let contract_address = {
        if contract_id.len() == 44 {
            hex::decode(contract_id)?
        } else if contract_id.len() > 3 {
            let mut contract_address = Vec::with_capacity(22);
            match &contract_id[0..3] {
                "tz1" => {
                    contract_address.extend(&[0, 0]);
                    contract_address.extend(ContractTz1Hash::try_from(contract_id)?.as_ref());
                }
                "tz2" => {
                    contract_address.extend(&[0, 1]);
                    contract_address.extend(ContractTz2Hash::try_from(contract_id)?.as_ref());
                }
                "tz3" => {
                    contract_address.extend(&[0, 2]);
                    contract_address.extend(ContractTz3Hash::try_from(contract_id)?.as_ref());
                }
                "KT1" => {
                    contract_address.push(1);
                    contract_address.extend(ContractKt1Hash::try_from(contract_id)?.as_ref());
                    contract_address.push(0);
                }
                _ => {
                    return Err(ConversionError::InvalidCurveTag {
                        curve_tag: contract_id.to_string(),
                    });
                }
            }
            contract_address
        } else {
            return Err(ConversionError::InvalidHash {
                hash: contract_id.to_string(),
            });
        }
    };

    Ok(contract_address)
}

pub(crate) async fn get_shell_automaton_state_current(
    env: &RpcServiceEnvironment,
) -> Result<shell_automaton::State, tokio::sync::oneshot::error::RecvError> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    let _ = env
        .shell_automaton_sender()
        .send(RpcShellAutomatonMsg::GetCurrentGlobalState { channel: tx })
        .await;
    rx.await
}

pub(crate) async fn get_shell_automaton_storage_requests(
    env: &RpcServiceEnvironment,
) -> Result<
    shell_automaton::service::rpc_service::StorageRequests,
    tokio::sync::oneshot::error::RecvError,
> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    let _ = env
        .shell_automaton_sender()
        .send(RpcShellAutomatonMsg::GetStorageRequests { channel: tx })
        .await;
    rx.await
}

pub(crate) async fn get_shell_automaton_state_after(
    env: &RpcServiceEnvironment,
    target_action_id: u64,
) -> anyhow::Result<shell_automaton::State> {
    let mut state = shell_automaton_state_closest(env, target_action_id).await?;

    let action_storage = ShellAutomatonActionStorage::new(env.persistent_storage());

    tokio::task::spawn_blocking(move || {
        let actions_iter = action_storage
            .find(IteratorMode::From(
                Cow::Owned(target_action_id),
                Direction::Forward,
            ))?
            .map(shell_automaton_actions_decode_map);

        for result in actions_iter {
            let action = result?;
            let action_id = u64::from(action.id);

            if action_id <= target_action_id {
                shell_automaton::reducer(&mut state, &action);
            }
            if action_id >= target_action_id {
                break;
            }
        }
        Ok(state)
    })
    .await?
}

#[allow(dead_code)]
pub(crate) async fn get_shell_automaton_action(
    env: &RpcServiceEnvironment,
    action_id: u64,
) -> anyhow::Result<Option<ActionWithMeta>> {
    let action_storage = ShellAutomatonActionStorage::new(env.persistent_storage());
    tokio::task::spawn_blocking(move || {
        Ok(action_storage
            .get::<Action>(&action_id)?
            .map(|action| ActionWithMeta {
                id: ActionId::new_unchecked(action_id),
                // TODO: include in the storage.
                depth: 0,
                action,
            }))
    })
    .await?
}

fn shell_automaton_actions_decode_map(
    result: Result<BoxedSliceKV, DBError>,
) -> Result<ActionWithMeta, StorageError> {
    let (key, value) = result?;
    Ok(ActionWithMeta {
        id: ActionId::new_unchecked(u64::decode(&key)?),
        // TODO: include in the storage.
        depth: 0,
        action: Action::decode(&value)?,
    })
}

pub(crate) async fn shell_automaton_state_closest(
    env: &RpcServiceEnvironment,
    target_action_id: u64,
) -> anyhow::Result<shell_automaton::State> {
    let snapshot_storage = ShellAutomatonStateStorage::new(env.persistent_storage());

    tokio::task::spawn_blocking(move || {
        match snapshot_storage.get_closest_before(&target_action_id)? {
            Some(v) => Ok(v),
            None => Err(anyhow::anyhow!("snapshot not available")),
        }
    })
    .await?
}

/// Action sent from rpc.
#[derive(Serialize, Deserialize)]
pub(crate) struct RpcShellAutomatonAction {
    #[serde(flatten)]
    pub action: ActionWithMeta,
    pub state: shell_automaton::State,
    /// Time between this action and the next one.
    pub duration: u64,
}

pub(crate) async fn get_shell_automaton_actions(
    env: &RpcServiceEnvironment,
    cursor: Option<u64>,
    limit: Option<usize>,
) -> anyhow::Result<VecDeque<RpcShellAutomatonAction>> {
    let snapshot_storage = ShellAutomatonStateStorage::new(env.persistent_storage());
    let action_storage = ShellAutomatonActionStorage::new(env.persistent_storage());

    let limit = limit.unwrap_or(20).max(1).min(1000);

    tokio::task::spawn_blocking(move || {
        let mut actions_iter = action_storage
            .find(match cursor {
                Some(cursor) => {
                    IteratorMode::From(Cow::Owned(cursor.max(1) - 1), Direction::Reverse)
                }
                None => IteratorMode::End,
            })?
            .map(shell_automaton_actions_decode_map);

        let mut result_actions = Vec::with_capacity(limit + 1);

        for _ in 0..=limit {
            match actions_iter.next().transpose()? {
                Some(action) => result_actions.push(action),
                None => break,
            }
        }

        let state: shell_automaton::State = match result_actions.last() {
            Some(first_action) => {
                match snapshot_storage.get_closest_before(&first_action.id.into())? {
                    Some(v) => Ok(v),
                    None => Err(anyhow::anyhow!("snapshot not available")),
                }?
            }
            None => return Ok(VecDeque::new()),
        };

        let mut actions_to_apply = vec![];

        for result in actions_iter {
            let action = result?;

            match action.id.cmp(&state.last_action.id()) {
                Ordering::Greater => {
                    actions_to_apply.push(action);
                }
                Ordering::Equal => {
                    actions_to_apply.push(action);
                    break;
                }
                Ordering::Less => {
                    break;
                }
            }
        }

        let state = actions_to_apply
            .into_iter()
            .rev()
            .fold(state, |mut state, action| {
                shell_automaton::reducer(&mut state, &action);
                state
            });

        let action_times = result_actions
            .iter()
            .rev()
            .map(|x| u64::from(x.id))
            .collect::<Vec<_>>();

        Ok(result_actions
            .into_iter()
            .rev()
            .enumerate()
            .take(limit)
            .fold(
                (state, VecDeque::with_capacity(limit)),
                |(mut state, mut result), (index, action)| {
                    let action_time = u64::from(action.id);
                    let next_action_time = action_times.get(index + 1).cloned().unwrap_or(0);

                    shell_automaton::reducer(&mut state, &action);
                    result.push_front(RpcShellAutomatonAction {
                        action,
                        state: state.clone(),
                        duration: next_action_time.saturating_sub(action_time),
                    });
                    (state, result)
                },
            )
            .1)
    })
    .await?
}

pub(crate) async fn get_shell_automaton_actions_raw(
    env: &RpcServiceEnvironment,
    cursor: Option<u64>,
    limit: Option<usize>,
) -> anyhow::Result<RpcShellAutomatonActionsRaw> {
    let actions_with_state = get_shell_automaton_actions(env, cursor, limit).await?;

    Ok(RpcShellAutomatonActionsRaw {
        actions: actions_with_state
            .iter()
            .map(|a| a.action.action.clone())
            .collect(),
        initial_state: actions_with_state[0].state.clone(),
    })
}

pub(crate) async fn get_shell_automaton_actions_reverse(
    env: &RpcServiceEnvironment,
    cursor: Option<u64>,
    limit: Option<usize>,
) -> anyhow::Result<VecDeque<RpcShellAutomatonAction>> {
    let action_storage = ShellAutomatonActionStorage::new(env.persistent_storage());

    let cursor = cursor.unwrap_or(0);
    let limit = limit.unwrap_or(20).max(1).min(1000);

    let mut state = shell_automaton_state_closest(env, cursor).await?;

    tokio::task::spawn_blocking(move || {
        let mut actions_iter = action_storage
            .find(IteratorMode::From(
                Cow::Owned(state.last_action.id().into()),
                Direction::Forward,
            ))?
            .map(shell_automaton_actions_decode_map)
            .peekable();

        loop {
            match actions_iter.peek() {
                Some(Ok(action)) if u64::from(action.id) >= cursor => break,
                None => break,
                _ => {}
            }
            if let Some(action) = actions_iter.next().transpose()? {
                shell_automaton::reducer(&mut state, &action);
            }
        }

        let mut result_actions = VecDeque::with_capacity(limit + 1);

        for _ in 0..=limit {
            match actions_iter.next().transpose()? {
                Some(action) => result_actions.push_front(action),
                None => break,
            }
        }

        let action_times = result_actions
            .iter()
            .rev()
            .map(|x| u64::from(x.id))
            .collect::<Vec<_>>();

        Ok(result_actions
            .into_iter()
            .rev()
            .enumerate()
            .take(limit)
            .fold(
                (state, VecDeque::with_capacity(limit)),
                |(mut state, mut result), (index, action)| {
                    let action_time = u64::from(action.id);
                    let next_action_time = action_times.get(index + 1).cloned().unwrap_or(0);

                    shell_automaton::reducer(&mut state, &action);
                    result.push_front(RpcShellAutomatonAction {
                        action,
                        state: state.clone(),
                        duration: next_action_time.saturating_sub(action_time),
                    });
                    (state, result)
                },
            )
            .1)
    })
    .await?
}

pub(crate) type ShellAutomatonActionsStats = HashMap<String, ShellAutomatonActionStatsForRanges>;

pub(crate) async fn get_shell_automaton_actions_stats(
    env: &RpcServiceEnvironment,
) -> anyhow::Result<ShellAutomatonActionsStats> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    let _ = env
        .shell_automaton_sender()
        .send(RpcShellAutomatonMsg::GetActionKindStats { channel: tx })
        .await;
    Ok(rx.await?.stats)
}

pub(crate) async fn get_shell_automaton_actions_stats_for_blocks(
    env: &RpcServiceEnvironment,
    level_filter: Option<BTreeSet<Level>>,
) -> anyhow::Result<Vec<ActionKindStatsForBlock>> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    let _ = env
        .shell_automaton_sender()
        .send(RpcShellAutomatonMsg::GetActionKindStatsForBlocks {
            channel: tx,
            level_filter,
        })
        .await;
    Ok(rx.await?)
}

pub(crate) async fn get_shell_automaton_actions_graph(
    env: &RpcServiceEnvironment,
) -> anyhow::Result<Vec<serde_json::Value>> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    let _ = env
        .shell_automaton_sender()
        .send(RpcShellAutomatonMsg::GetActionGraph { channel: tx })
        .await;
    Ok(rx
        .await?
        .into_iter()
        .enumerate()
        .map(|(action_id, node)| {
            serde_json::json!({
                "actionId": action_id,
                "actionKind": node.action_kind,
                "nextActions": node.next_actions,
            })
        })
        .collect())
}

pub type OperationsStats = BTreeMap<String, OperationStats>;

#[derive(Serialize)]
pub struct OperationStats {
    kind: Option<OperationKind>,
    /// Minimum time when we saw this operation. Latencies are measured
    /// from this point.
    min_time: Option<u64>,
    first_block_timestamp: Option<u64>,
    validation_started: Option<i128>,
    validation_result: Option<(
        i128,
        shell_automaton::mempool::OperationValidationResult,
        Option<i128>,
        Option<i128>,
    )>,
    validations: Vec<OperationValidationStats>,
    nodes: HashMap<String, OperationNodeStats>,
    injected_timestamp: Option<u64>,
    current_heads: BTreeSet<BlockHash>,
}

#[derive(Serialize)]
pub struct OperationNodeStats {
    received: Vec<OperationNodeCurrentHeadStats>,
    sent: Vec<OperationNodeCurrentHeadStats>,

    content_requested: Vec<i128>,
    content_received: Vec<i128>,

    content_requested_remote: Vec<i128>,
    content_sent: Vec<i128>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OperationNodeCurrentHeadStats {
    /// Latency from first time we have seen that operation.
    latency: i128,
    block_level: i32,
    block_timestamp: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OperationValidationStats {
    started: Option<i128>,
    finished: Option<i128>,
    preapply_started: Option<i128>,
    preapply_ended: Option<i128>,
    current_head_level: Option<i32>,
    result: Option<OperationValidationResult>,
}

pub(crate) async fn get_shell_automaton_mempool_operation_stats(
    env: &RpcServiceEnvironment,
    filter: shell_automaton::service::rpc_service::MempoolOperationStatsFilter,
) -> Result<OperationsStats, tokio::sync::oneshot::error::RecvError> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    let _ = env
        .shell_automaton_sender()
        .send(RpcShellAutomatonMsg::GetMempoolOperationStats {
            channel: tx,
            filter,
        })
        .await;

    let result = rx.await?;

    Ok(map_operations_stats(result))
}

pub type BlocksStats = Vec<BlockStats>;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlockStats {
    /// First time(ns) we saw a block in current head.
    block_first_seen: u64,

    block_level: Level,
    block_round: Option<i32>,

    /// Time(ns) since block_first_seen in current head.
    data_ready: Option<u64>,

    /// Time(ns) it took to load data from storage.
    load_data: Option<u64>,

    /// Time(ns) it took for protocol runner to apply block.
    apply_block: Option<u64>,

    /// Time(ns) it took to store the result.
    store_result: Option<u64>,

    /// Injection time
    injected: Option<u64>,
}

pub(crate) async fn get_shell_automaton_block_stats_graph(
    env: &RpcServiceEnvironment,
    limit: Option<usize>,
) -> Result<Option<BlocksStats>, tokio::sync::oneshot::error::RecvError> {
    let limit = limit.unwrap_or(2000);

    let (tx, rx) = tokio::sync::oneshot::channel();

    let _ = env
        .shell_automaton_sender()
        .send(RpcShellAutomatonMsg::GetBlockStats { channel: tx })
        .await;

    let stats = match rx.await? {
        Some(v) => v,
        None => return Ok(None),
    };

    let times_sub = |a: Option<u64>, b: Option<u64>| a.and_then(|a| b.map(|b| a.saturating_sub(b)));

    let mut result = stats
        .into_iter()
        .map(|(_, stats)| {
            let block_first_seen = stats.receive_timestamp;
            let data_ready_start = stats.receive_timestamp;

            BlockStats {
                block_first_seen,
                block_level: stats.level,
                block_round: stats.round,
                data_ready: times_sub(stats.load_data_start, Some(data_ready_start)),
                load_data: times_sub(stats.load_data_end, stats.load_data_start),
                apply_block: times_sub(stats.apply_block_end, stats.apply_block_start),
                store_result: times_sub(stats.store_result_end, stats.store_result_start),
                injected: stats.injected,
            }
        })
        .collect::<Vec<_>>();

    result.sort_by(|a, b| {
        a.block_level
            .cmp(&b.block_level)
            .then(a.block_round.cmp(&b.block_round))
    });
    // take last/newest `limit` block stats.
    result = result.into_iter().rev().take(limit).rev().collect();

    Ok(Some(result))
}

pub(crate) async fn get_shell_automaton_baking_rights(
    block_hash: BlockHash,
    level: Option<Level>,
    env: &RpcServiceEnvironment,
) -> anyhow::Result<serde_json::Value> {
    let rx = env
        .shell_automaton_sender()
        .send(RpcShellAutomatonMsg::GetBakingRights { block_hash, level })
        .await?;

    let response = rx.await?;
    Ok(response)
}

pub(crate) async fn get_shell_automaton_endorsing_rights(
    block_hash: BlockHash,
    level: Option<Level>,
    env: &RpcServiceEnvironment,
) -> anyhow::Result<serde_json::Value> {
    let rx = env
        .shell_automaton_sender()
        .send(RpcShellAutomatonMsg::GetEndorsingRights { block_hash, level })
        .await?;

    let response = rx.await?;
    Ok(response)
}

fn adjust_times(base_time: u64, value: &mut serde_json::Value) -> anyhow::Result<()> {
    let obj = value
        .as_object_mut()
        .ok_or_else(|| anyhow::format_err!("object expected"))?;
    obj.iter_mut().for_each(|(_, v)| {
        if let Some(v) = v.as_object_mut() {
            v.iter_mut().for_each(|(k, v)| {
                if k.ends_with("_time") {
                    if let Some(t) = v.as_i64() {
                        *v = serde_json::json!(t - (base_time as i64));
                    }
                }
            })
        }
    });
    Ok(())
}

pub(crate) async fn get_shell_automaton_endorsements_status(
    block_hash: Option<BlockHash>,
    payload_hash: Option<BlockPayloadHash>,
    level: Option<i32>,
    round: Option<i32>,
    base_time: Option<u64>,
    env: &RpcServiceEnvironment,
) -> anyhow::Result<serde_json::Value> {
    use shell_automaton::mempool::mempool_actions::ConsensusOperationMatcher;
    let matcher = match (block_hash, payload_hash, level, round) {
        (Some(block_hash), None, None, None) => {
            ConsensusOperationMatcher::endorsement_branch(block_hash)
        }
        (None, None, Some(level), round) => {
            ConsensusOperationMatcher::endorsement_level_round(level, round)
        }
        _ => anyhow::bail!("Either `block_hash` or `level`+`round` should be specified"),
    };
    let rx = env
        .shell_automaton_sender()
        .send(RpcShellAutomatonMsg::GetEndorsementsStatus { matcher })
        .await?;

    let mut response = rx.await?;
    if let Some(base_time) = base_time {
        adjust_times(base_time, &mut response)?;
    }
    Ok(response)
}

pub(crate) async fn get_shell_automaton_preendorsements_status(
    payload_hash: Option<BlockPayloadHash>,
    level: Option<i32>,
    round: Option<i32>,
    base_time: Option<u64>,
    env: &RpcServiceEnvironment,
) -> anyhow::Result<serde_json::Value> {
    use shell_automaton::mempool::mempool_actions::ConsensusOperationMatcher;
    let matcher = match (payload_hash, level, round) {
        (None, Some(level), round) => {
            ConsensusOperationMatcher::preendorsement_level_round(level, round)
        }
        _ => anyhow::bail!("`level`+`round` should be specified"),
    };
    let rx = env
        .shell_automaton_sender()
        .send(RpcShellAutomatonMsg::GetEndorsementsStatus { matcher })
        .await?;

    let mut response = rx.await?;
    if let Some(base_time) = base_time {
        adjust_times(base_time, &mut response)?;
    }
    Ok(response)
}

pub(crate) async fn get_shell_automaton_stats_current_head(
    level: i32,
    round: Option<i32>,
    env: &RpcServiceEnvironment,
) -> anyhow::Result<Vec<(BlockHash, shell_automaton::service::BlockApplyStats)>> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let _ = env
        .shell_automaton_sender()
        .send(RpcShellAutomatonMsg::GetStatsCurrentHeadStats {
            channel: tx,
            level,
            round,
        })
        .await?;
    let response = rx.await?;
    Ok(response)
}

pub(crate) async fn get_shell_automaton_endrosement_stats(
    env: &RpcServiceEnvironment,
) -> anyhow::Result<BTreeMap<String, OperationStats>> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let _ = env
        .shell_automaton_sender()
        .send(RpcShellAutomatonMsg::GetMempooEndrosementsStats { channel: tx })
        .await?;

    // TODO: this is a duplicate, move it to a function
    let raw_result = rx.await?;

    Ok(map_operations_stats(raw_result))
}

fn map_operations_stats(
    raw: BTreeMap<OperationHash, shell_automaton::mempool::OperationStats>,
) -> BTreeMap<String, OperationStats> {
    raw.into_iter()
        .map(|(op_hash, op_stats)| {
            let start_time = op_stats
                .first_block_timestamp
                // convert from seconds to nanoseconds.
                .and_then(|v| v.checked_mul(1_000_000_000))
                .unwrap_or(0) as i128;

            let op_stats = OperationStats {
                injected_timestamp: op_stats.injected_timestamp,
                kind: op_stats.kind,
                min_time: op_stats.min_time,
                first_block_timestamp: op_stats.first_block_timestamp,
                validation_started: op_stats
                    .validation_started
                    .map(|t| (t as i128).checked_sub(start_time).unwrap_or(0)),
                validation_result: op_stats.validation_result.map(
                    |(t, result, preapply_started, preapply_ended)| {
                        (
                            (t as i128).checked_sub(start_time).unwrap_or(0),
                            result,
                            preapply_started.map(|preapply_started| {
                                (preapply_started as i128)
                                    .checked_sub(start_time)
                                    .unwrap_or(0)
                            }),
                            preapply_ended.map(|preapply_ended| {
                                (preapply_ended as i128)
                                    .checked_sub(start_time)
                                    .unwrap_or(0)
                            }),
                        )
                    },
                ),
                validations: op_stats
                    .validations
                    .into_iter()
                    .map(|v| OperationValidationStats {
                        started: v
                            .started
                            .map(|t| (t as i128).checked_sub(start_time).unwrap_or(0)),
                        finished: v
                            .finished
                            .map(|t| (t as i128).checked_sub(start_time).unwrap_or(0)),
                        preapply_started: v
                            .preapply_started
                            .map(|t| (t as i128).checked_sub(start_time).unwrap_or(0)),
                        preapply_ended: v
                            .preapply_ended
                            .map(|t| (t as i128).checked_sub(start_time).unwrap_or(0)),
                        current_head_level: v.current_head_level,
                        result: v.result,
                    })
                    .collect(),
                nodes: op_stats
                    .nodes
                    .into_iter()
                    .map(|(k, stats)| {
                        (
                            k.to_base58_check(),
                            OperationNodeStats {
                                received: stats
                                    .received
                                    .into_iter()
                                    .map(|stats| OperationNodeCurrentHeadStats {
                                        latency: (stats.time as i128)
                                            .checked_sub(start_time)
                                            .unwrap_or(0),
                                        block_level: stats.block_level,
                                        block_timestamp: stats.block_timestamp,
                                    })
                                    .collect(),
                                sent: stats
                                    .sent
                                    .into_iter()
                                    .map(|stats| OperationNodeCurrentHeadStats {
                                        latency: (stats.time as i128)
                                            .checked_sub(start_time)
                                            .unwrap_or(0),
                                        block_level: stats.block_level,
                                        block_timestamp: stats.block_timestamp,
                                    })
                                    .collect(),
                                content_requested: stats
                                    .content_requested
                                    .into_iter()
                                    .map(|t| (t as i128).checked_sub(start_time).unwrap_or(0))
                                    .collect(),
                                content_received: stats
                                    .content_received
                                    .into_iter()
                                    .map(|t| (t as i128).checked_sub(start_time).unwrap_or(0))
                                    .collect(),
                                content_requested_remote: stats
                                    .content_requested_remote
                                    .into_iter()
                                    .map(|t| (t as i128).checked_sub(start_time).unwrap_or(0))
                                    .collect(),
                                content_sent: stats
                                    .content_sent
                                    .into_iter()
                                    .map(|t| (t as i128).checked_sub(start_time).unwrap_or(0))
                                    .collect(),
                            },
                        )
                    })
                    .collect(),
                current_heads: op_stats.current_heads,
            };

            (op_hash.to_base58_check(), op_stats)
        })
        .collect()
}

pub(crate) async fn get_shell_automaton_baking_state(
    env: &RpcServiceEnvironment,
) -> Option<BakingState> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let _ = env
        .shell_automaton_sender()
        .send(RpcShellAutomatonMsg::GetBakingState { channel: tx })
        .await;

    rx.await.ok()?
}

// get_best_remote_level
pub(crate) async fn get_best_remote_level(
    env: &RpcServiceEnvironment,
) -> anyhow::Result<Option<i32>> {
    if let Ok(rpc_state) = env.state().read() {
        Ok(*rpc_state.best_remote_level())
    } else {
        Ok(None)
    }
}
