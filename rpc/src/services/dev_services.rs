// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

// TODO - TE-261: many things commented out here because they don't work with the new
// context until we reintroduce something equivalent to the context actions storage.
// The timings database, along with the readonly IPC context access could be used
// to reproduce the same functionality.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet, VecDeque};
use std::convert::TryFrom;
use std::future::Future;
use std::vec;

use crypto::hash::ContractKt1Hash;
use serde::{Deserialize, Serialize};
use shell_automaton::{Action, ActionWithId};
use slog::Logger;

use crypto::hash::{BlockHash, ChainId, ContractTz1Hash, ContractTz2Hash, ContractTz3Hash};
use shell::stats::memory::{Memory, MemoryData, MemoryStatsResult};
use shell_automaton::service::rpc_service::RpcResponse as RpcShellAutomatonMsg;
use shell_automaton::{ActionId, ActionType};
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

    format!("v{}", version_env.to_string())
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

pub(crate) async fn get_shell_automaton_state_after(
    env: &RpcServiceEnvironment,
    target_action_id: u64,
) -> anyhow::Result<shell_automaton::State> {
    let mut state = shell_automaton_state_closest(env, target_action_id).await?;

    let action_storage = ShellAutomatonActionStorage::new(env.persistent_storage());
    let actions_iter = shell_automaton_actions_iter(
        &action_storage,
        IteratorMode::From(Cow::Owned(target_action_id), Direction::Forward),
    )
    .await?
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
}

#[allow(dead_code)]
pub(crate) async fn get_shell_automaton_action(
    env: &RpcServiceEnvironment,
    action_id: u64,
) -> anyhow::Result<Option<ActionWithId<Action>>> {
    let action_storage = ShellAutomatonActionStorage::new(env.persistent_storage());
    tokio::task::spawn_blocking(move || {
        Ok(action_storage
            .get::<Action>(&action_id)?
            .map(|action| ActionWithId {
                id: ActionId::new_unchecked(action_id),
                action,
            }))
    })
    .await?
}

fn shell_automaton_actions_decode_map(
    result: Result<BoxedSliceKV, DBError>,
) -> Result<ActionWithId<Action>, StorageError> {
    let (key, value) = result?;
    Ok(ActionWithId {
        id: ActionId::new_unchecked(u64::decode(&key)?),
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

pub(crate) fn shell_automaton_actions_iter<'a>(
    action_storage: &'a ShellAutomatonActionStorage,
    mode: IteratorMode<'a, ShellAutomatonActionStorage>,
) -> impl 'a + Future<Output = anyhow::Result<impl 'a + Iterator<Item = Result<BoxedSliceKV, DBError>>>>
{
    async move {
        // TODO: tmp, shouldn't use block_in_place as it blocks tokio's
        // non-blocking worker thread.
        Ok::<_, anyhow::Error>(tokio::task::block_in_place(move || {
            Ok(action_storage.find(mode)?)
        }))?
    }
}

/// Action sent from rpc.
#[derive(Serialize, Deserialize)]
pub(crate) struct RpcShellAutomatonAction {
    #[serde(flatten)]
    action: ActionWithId<Action>,
    state: shell_automaton::State,
    /// Time between this action and the next one.
    duration: u64,
}

pub(crate) async fn get_shell_automaton_actions(
    env: &RpcServiceEnvironment,
    cursor: Option<u64>,
    limit: Option<usize>,
) -> anyhow::Result<VecDeque<RpcShellAutomatonAction>> {
    let action_storage = ShellAutomatonActionStorage::new(env.persistent_storage());

    let limit = limit.unwrap_or(20).max(1).min(1000);

    let mut actions_iter = shell_automaton_actions_iter(
        &action_storage,
        match cursor {
            Some(cursor) => IteratorMode::From(Cow::Owned(cursor - 1), Direction::Reverse),
            None => IteratorMode::End,
        },
    )
    .await?
    .map(shell_automaton_actions_decode_map);

    let mut result_actions = Vec::with_capacity(limit + 1);

    for _ in 0..=limit {
        match actions_iter.next().transpose()? {
            Some(action) => result_actions.push(action),
            None => break,
        }
    }

    let state = match result_actions.last() {
        Some(first_action) => shell_automaton_state_closest(env, first_action.id.into()).await?,
        None => return Ok(VecDeque::new()),
    };

    let mut actions_to_apply = vec![];

    for result in actions_iter {
        let action = result?;

        if action.id > state.last_action_id {
            actions_to_apply.push(action);
        } else if action.id == state.last_action_id {
            actions_to_apply.push(action);
            break;
        } else {
            break;
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
                    duration: next_action_time.checked_sub(action_time).unwrap_or(0),
                });
                (state, result)
            },
        )
        .1)
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

    let mut actions_iter = shell_automaton_actions_iter(
        &action_storage,
        IteratorMode::From(Cow::Owned(state.last_action_id.into()), Direction::Forward),
    )
    .await?
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
                result.push_back(RpcShellAutomatonAction {
                    action,
                    state: state.clone(),
                    duration: next_action_time.checked_sub(action_time).unwrap_or(0),
                });
                (state, result)
            },
        )
        .1)
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionGraphNode {
    action_id: usize,
    action_name: ActionType,
    next_actions: Vec<usize>,
}

pub(crate) async fn get_shell_automaton_actions_graph(
    env: &RpcServiceEnvironment,
) -> anyhow::Result<Vec<ActionGraphNode>> {
    let action_storage = ShellAutomatonActionStorage::new(env.persistent_storage());

    let mut action_indices = HashMap::new();
    let mut next_actions = Vec::new();

    let mut action_it = shell_automaton_actions_iter(&action_storage, IteratorMode::Start)
        .await?
        .map(shell_automaton_actions_decode_map)
        .map(|result| result.map(|action| action.into()));
    let action = action_it.next().unwrap()?;
    action_indices.insert(action, 0);
    next_actions.push(HashSet::new());
    let mut pred_action_index = 0;

    for result in action_it {
        let action = result?;
        let action_index = if let Some(action_index) = action_indices.get(&action) {
            *action_index
        } else {
            let action_index = action_indices.len();
            action_indices.insert(action, action_index);
            next_actions.push(HashSet::new());
            action_index
        };
        next_actions[pred_action_index].insert(action_index);
        pred_action_index = action_index;
    }

    let mut actions_graph = action_indices.into_iter().collect::<Vec<_>>();
    actions_graph.sort_by_key(|(_, k)| *k);
    let actions_graph = actions_graph
        .into_iter()
        .enumerate()
        .map(|(i, (s, i2))| {
            assert_eq!(i, i2);
            let mut next_actions: Vec<_> = next_actions[i].iter().cloned().collect();
            next_actions.sort();
            ActionGraphNode {
                action_id: i,
                action_name: s,
                next_actions,
            }
        })
        .collect::<Vec<_>>();

    Ok(actions_graph)
}
