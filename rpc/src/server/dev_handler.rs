// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::helpers::{parse_block_hash, parse_chain_id, RpcServiceError, MAIN_CHAIN_ID};
use crate::result_option_to_json_response;
use crate::server::{HasSingleValue, Params, Query, RpcServiceEnvironment};
use crate::services::rewards_services::CycleRewardsFilter;
use crate::services::{context, dev_services, rewards_services};
use crate::{empty, make_json_response, required_param, result_to_json_response, ServiceResult};
use anyhow::format_err;
use crypto::hash::{BlockHash, CryptoboxPublicKeyHash, OperationHash};
use crypto::PublicKeyWithHash;
use hyper::{Body, Request, Response};
use shell_automaton::service::{BlockApplyStats, BlockPeerStats};
use slog::warn;
use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::sync::Arc;
use storage::persistent::Encoder;

pub async fn dev_blocks(
    _: Request<Body>,
    _: Params,
    query: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    // TODO: TE-221 - add optional chain_id to params mapping
    let chain_id_param = MAIN_CHAIN_ID;
    let chain_id = parse_chain_id(chain_id_param, &env)?;

    // get block from params or fallback to current_head/genesis
    let from_block_id = match query.get_str("from_block_id") {
        Some(block_id_param) => parse_block_hash(&chain_id, block_id_param, &env).map_err(|e| {
            format_err!(
                "Failed to parse_block_hash, block_id_param: {}, reason: {}",
                block_id_param,
                e
            )
        })?,
        None => {
            // fallback, if no block param is present - check current head, if no one, then genesis
            let state = env
                .state()
                .read()
                .map_err(|e| format_err!("Failed to lock current state, reason: {}", e))?;
            state.current_head().hash.clone()
        }
    };

    // get cycle length
    let cycle_length =
        dev_services::get_cycle_length_for_block(&chain_id, &from_block_id, &env, env.log())
            .map_err(|e| format_err!("Failed to get cycle length, reason: {}", e))?;
    let every_nth_level = match query.get_str("every_nth") {
        Some("cycle") => Some(cycle_length),
        Some("voting-period") => Some(cycle_length * 8),
        _ => None,
    };
    let limit = query.get_usize("limit").unwrap_or(50);

    result_to_json_response(
        dev_services::get_blocks(chain_id, from_block_id, every_nth_level, limit, &env).await,
        env.log(),
    )
}

#[allow(dead_code)]
pub async fn dev_block_actions(
    _: Request<Body>,
    params: Params,
    _: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    // TODO: TE-221 - add optional chain_id to params mapping
    let chain_id_param = MAIN_CHAIN_ID;
    let chain_id = parse_chain_id(chain_id_param, &env)?;
    let block_hash = parse_block_hash(&chain_id, required_param!(params, "block_hash")?, &env)
        .map_err(|e| format_err!("Failed to parse_block_hash, reason: {}", e))?;
    result_to_json_response(
        dev_services::get_block_actions(block_hash, env.persistent_storage()),
        env.log(),
    )
}

#[allow(dead_code)]
pub async fn dev_contract_actions(
    _: Request<Body>,
    params: Params,
    query: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    let contract_id = required_param!(params, "contract_address")?;
    let from_id = query.get_u64("from_id");
    let limit = query.get_usize("limit").unwrap_or(50);
    result_to_json_response(
        dev_services::get_contract_actions(contract_id, from_id, limit, env.persistent_storage()),
        env.log(),
    )
}

pub async fn dev_db_stats(
    _: Request<Body>,
    _params: Params,
    _query: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    result_to_json_response(Ok(env.persistent_storage.main_db().db_stats()), env.log())
}

pub async fn dev_action_cursor(
    _: Request<Body>,
    params: Params,
    query: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    let cursor_id = query.get_u64("cursor_id");
    let limit = query.get_u64("limit").map(|limit| limit as usize);
    let action_types = query.get_str("action_types");

    result_to_json_response(
        if let Some(block_hash_param) = params.get_str("block_hash") {
            // TODO: TE-221 - add optional chain_id to params mapping
            let chain_id_param = MAIN_CHAIN_ID;
            let chain_id = parse_chain_id(chain_id_param, &env)?;
            let block_hash = parse_block_hash(&chain_id, block_hash_param, &env).map_err(|e| {
                format_err!(
                    "Failed to parse_block_hash, block_hash_param: {}, reason: {}",
                    block_hash_param,
                    e
                )
            })?;

            dev_services::get_block_actions_cursor(
                block_hash,
                cursor_id,
                limit,
                action_types,
                env.persistent_storage(),
            )
        } else if let Some(contract_address) = params.get_str("contract_address") {
            dev_services::get_contract_actions_cursor(
                contract_address,
                cursor_id,
                limit,
                action_types,
                env.persistent_storage(),
            )
        } else {
            Err(RpcServiceError::InvalidParameters {
                reason: "Invalid parameter: should be either `block_hash` or `contract_address`"
                    .to_string(),
            })
        },
        env.log(),
    )
}

pub async fn block_action_details(
    _: Request<Body>,
    params: Params,
    _: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    let chain_id_param = MAIN_CHAIN_ID;
    let chain_id = parse_chain_id(chain_id_param, &env)?;
    let block_hash = parse_block_hash(&chain_id, required_param!(params, "block_hash")?, &env)
        .map_err(|e| format_err!("Failed to parse_block_hash, reason: {}", e))?;
    result_to_json_response(
        dev_services::get_block_action_details(block_hash, env.persistent_storage()),
        env.log(),
    )
}

#[allow(dead_code)]
pub async fn dev_stats_storage(
    _: Request<Body>,
    _: Params,
    _: Query,
    _env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    // TODO - TE-261: disabled for now because we don't have the context actions database
    // result_to_json_response(
    //     crate::services::stats_services::compute_storage_stats(
    //         env.state(),
    //         env.main_chain_genesis_hash(),
    //         env.persistent_storage(),
    //     ),
    //     env.log(),
    // )
    empty()
}

pub async fn dev_stats_memory(
    _: Request<Body>,
    _: Params,
    _: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    match dev_services::get_stats_memory() {
        Ok(resp) => make_json_response(&resp),
        Err(e) => {
            warn!(env.log(), "GetStatsMemory: {}", e);
            empty()
        }
    }
}

pub async fn dev_stats_memory_protocol_runners(
    _: Request<Body>,
    _: Params,
    _: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    match dev_services::get_stats_memory_protocol_runners() {
        Ok(resp) => make_json_response(&resp),
        Err(e) => {
            warn!(env.log(), "GetStatsMemory: {}", e);
            empty()
        }
    }
}

pub async fn context_stats(
    _: Request<Body>,
    _: Params,
    query: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    let context_name = query.get_str("context_name").unwrap_or("tezedge");
    let protocol = query.get_str("protocol");
    let db_path = env.context_stats_db_path.as_ref();

    result_to_json_response(
        context::make_context_stats(db_path, context_name, protocol),
        env.log(),
    )
}

pub async fn block_actions(
    _: Request<Body>,
    params: Params,
    _: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    let chain_id = parse_chain_id(required_param!(params, "chain_id")?, &env)?;
    let block_hash = parse_block_hash(&chain_id, required_param!(params, "block_id")?, &env)
        .map_err(|e| format_err!("Failed to parse_block_hash, reason: {}", e))?;
    let db_path = env.context_stats_db_path.as_ref();

    result_option_to_json_response(context::make_block_stats(db_path, block_hash), env.log())
}

pub async fn cycle_eras(
    _: Request<Body>,
    params: Params,
    _: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    let chain_id = parse_chain_id(required_param!(params, "chain_id")?, &env)?;
    let block_hash = parse_block_hash(&chain_id, required_param!(params, "block_id")?, &env)
        .map_err(|e| format_err!("Failed to parse_block_hash, reason: {}", e))?;

    result_to_json_response(
        dev_services::get_cycle_eras(&chain_id, &block_hash, &env, env.log()),
        env.log(),
    )
}

/// Get the version string
pub async fn dev_version(
    _: Request<Body>,
    _: Params,
    _: Query,
    _: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    make_json_response(&dev_services::get_dev_version())
}

pub async fn dev_shell_automaton_state_get(
    _: Request<Body>,
    _: Params,
    query: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    match query.get_u64("action_id") {
        Some(target_action_id) => make_json_response(
            &dev_services::get_shell_automaton_state_after(&env, target_action_id).await?,
        ),
        None => make_json_response(&dev_services::get_shell_automaton_state_current(&env).await?),
    }
}

pub async fn dev_shell_automaton_state_raw_get(
    _: Request<Body>,
    _: Params,
    _query: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    let state = dev_services::get_shell_automaton_state_current(&env).await?;
    let contents = state.encode()?;

    Ok(Response::builder()
        .header(hyper::header::CONTENT_TYPE, "application/octet-stream")
        .header(hyper::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(hyper::header::ACCESS_CONTROL_ALLOW_HEADERS, "Content-Type")
        .header(hyper::header::ACCESS_CONTROL_ALLOW_HEADERS, "content-type")
        .header(
            hyper::header::ACCESS_CONTROL_ALLOW_METHODS,
            "GET, POST, OPTIONS, PUT",
        )
        .body(Body::from(contents))?)
}

pub async fn dev_shell_automaton_actions_raw_get(
    _: Request<Body>,
    _: Params,
    query: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    let actions = dev_services::get_shell_automaton_actions_raw(
        &env,
        query.get_u64("cursor"),
        query.get_usize("limit"),
    )
    .await?;

    let contents = actions.encode()?;

    Ok(Response::builder()
        .header(hyper::header::CONTENT_TYPE, "application/octet-stream")
        .header(hyper::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(hyper::header::ACCESS_CONTROL_ALLOW_HEADERS, "Content-Type")
        .header(hyper::header::ACCESS_CONTROL_ALLOW_HEADERS, "content-type")
        .header(
            hyper::header::ACCESS_CONTROL_ALLOW_METHODS,
            "GET, POST, OPTIONS, PUT",
        )
        .body(Body::from(contents))?)
}

pub async fn dev_shell_automaton_storage_requests_get(
    _: Request<Body>,
    _: Params,
    _: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    make_json_response(&dev_services::get_shell_automaton_storage_requests(&env).await?)
}

pub async fn dev_shell_automaton_actions_get(
    _: Request<Body>,
    _: Params,
    query: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    make_json_response(&match query.get_usize("rev").eq(&Some(1)) {
        false => {
            dev_services::get_shell_automaton_actions(
                &env,
                query.get_u64("cursor"),
                query.get_usize("limit"),
            )
            .await?
        }
        true => {
            dev_services::get_shell_automaton_actions_reverse(
                &env,
                query.get_u64("cursor"),
                query.get_usize("limit"),
            )
            .await?
        }
    })
}

pub async fn dev_shell_automaton_actions_stats_get(
    _: Request<Body>,
    _: Params,
    _: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    make_json_response(&dev_services::get_shell_automaton_actions_stats(&env).await?)
}

pub async fn dev_shell_automaton_actions_stats_for_blocks_get(
    _: Request<Body>,
    _: Params,
    query: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    let level_filter = query.get("level").map(|v| {
        v.iter()
            .flat_map(|s| s.split(','))
            .filter_map(|s| s.parse().ok())
            .take(64)
            .collect::<BTreeSet<_>>()
    });
    make_json_response(
        &dev_services::get_shell_automaton_actions_stats_for_blocks(&env, level_filter).await?,
    )
}

pub async fn dev_shell_automaton_actions_graph_get(
    _: Request<Body>,
    _: Params,
    _: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    make_json_response(&dev_services::get_shell_automaton_actions_graph(&env).await?)
}

pub async fn dev_shell_automaton_mempool_operation_stats_get(
    _: Request<Body>,
    _: Params,
    query: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    use shell_automaton::service::rpc_service::MempoolOperationStatsFilter;
    let hash_filter = query
        .get("hash")
        .map(|v| {
            v.iter()
                .flat_map(|s| s.split(','))
                .filter_map(|s| OperationHash::from_base58_check(s).ok())
                .collect::<BTreeSet<_>>()
        })
        .filter(|v| !v.is_empty());
    let head_filter = query.get_hash("head")?;
    let filter = match (hash_filter, head_filter) {
        (None, None) => MempoolOperationStatsFilter::None,
        (Some(v), None) => MempoolOperationStatsFilter::OperationHashes(v),
        (None, Some(v)) => MempoolOperationStatsFilter::BlockHash(v),
        _ => return Err(anyhow::anyhow!("Either `hashes` or `head` is expected").into()),
    };
    make_json_response(
        &dev_services::get_shell_automaton_mempool_operation_stats(&env, filter).await?,
    )
}

pub async fn dev_shell_automaton_block_stats_graph_get(
    _: Request<Body>,
    _: Params,
    query: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    let limit = query.get_usize("limit");
    make_json_response(&dev_services::get_shell_automaton_block_stats_graph(&env, limit).await?)
}

pub async fn dev_shell_automaton_baking_rights(
    _: Request<Body>,
    _: Params,
    query: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    let block_hash = query
        .get_str("block")
        .ok_or_else(|| anyhow::anyhow!("Missing mandatory query parameter `block`"))?;
    let block_hash = BlockHash::from_base58_check(block_hash)?;
    let level = query.get_str("level").map(str::parse).transpose()?;
    make_json_response(
        &dev_services::get_shell_automaton_baking_rights(block_hash, level, &env).await?,
    )
}

pub async fn dev_shell_automaton_endorsing_rights(
    _: Request<Body>,
    _: Params,
    query: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    let block_hash = query
        .get_str("block")
        .ok_or_else(|| anyhow::anyhow!("Missing mandatory query parameter `block`"))?;
    let block_hash = BlockHash::from_base58_check(block_hash)?;
    let level = query.get_str("level").map(str::parse).transpose()?;
    make_json_response(
        &dev_services::get_shell_automaton_endorsing_rights(block_hash, level, &env).await?,
    )
}

pub async fn dev_shell_automaton_endorsements_status(
    _: Request<Body>,
    _: Params,
    query: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    let block_hash = query.get_hash("block_hash")?;
    let payload_hash = query.get_hash("payload_hash")?;
    let level = query.get_parsed("level")?;
    let round = query.get_parsed("round")?;
    let base_time = query.get_parsed("base_time")?;
    make_json_response(
        &dev_services::get_shell_automaton_endorsements_status(
            block_hash,
            payload_hash,
            level,
            round,
            base_time,
            &env,
        )
        .await?,
    )
}

pub async fn dev_shell_automaton_preendorsements_status(
    _: Request<Body>,
    _: Params,
    query: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    let payload_hash = query.get_hash("payload_hash")?;
    let level = query.get_parsed("level")?;
    let round = query.get_parsed("round")?;
    let base_time = query.get_parsed("base_time")?;
    make_json_response(
        &dev_services::get_shell_automaton_preendorsements_status(
            payload_hash,
            level,
            round,
            base_time,
            &env,
        )
        .await?,
    )
}

fn application_stats(hash: BlockHash, stats: BlockApplyStats, base_time: u64) -> serde_json::Value {
    let as_delta = |time: u64| time.saturating_sub(base_time);
    let as_delta_or = |time: Option<u64>| time.map_or(0, as_delta);

    let first_send_end_time = stats
        .peers
        .iter()
        .filter_map(|(_, s)| s.head_send_end.first())
        .max();

    let protocol_times = stats
        .apply_block_stats
        .map(|abs| {
            serde_json::json!({
                "apply_start": as_delta(abs.apply_start),

                "operations_decoding_start": as_delta(abs.operations_decoding_start),
                "operations_decoding_end": as_delta(abs.operations_decoding_end),

                "operations_metadata_encoding_start": as_delta(abs.operations_metadata_encoding_start),
                    "operations_metadata_encoding_end": as_delta(abs.operations_metadata_encoding_end),

                "begin_application_start": as_delta(abs.begin_application_start),
                "begin_application_end": as_delta(abs.begin_application_end),

                "finalize_block_start": as_delta(abs.finalize_block_start),
                "finalize_block_end": as_delta(abs.finalize_block_end),

                "collect_new_rolls_owner_snapshots_start": as_delta(abs.collect_new_rolls_owner_snapshots_start),
                "collect_new_rolls_owner_snapshots_end": as_delta(abs.collect_new_rolls_owner_snapshots_end),

                "commit_start": as_delta(abs.commit_start),
                "commit_end": as_delta(abs.commit_end),

                "apply_end": as_delta(abs.apply_end),
            })
        })
        .unwrap_or_default();
    serde_json::json!({
        "block_hash": hash,
        "block_level": stats.level,
        "block_round": stats.round,
        "block_timestamp": stats.block_timestamp.saturating_mul(1_000_000_000),
        "receive_timestamp": stats.receive_timestamp,
        "injected": stats.injected,

        "baker": stats.baker.and_then(|baker| baker.pk_hash().ok()).map(|pkh| pkh.to_string_representation()),
        "baker_priority": stats.priority,

        "round": stats.round,
        "payload_hash": stats.payload_hash,
        "payload_round": stats.payload_round,

        "precheck_start": as_delta_or(stats.precheck_start),
        "precheck_end": as_delta_or(stats.precheck_end),

        "download_block_header_start": as_delta_or(stats.download_block_header_start),
        "download_block_header_end": as_delta_or(stats.download_block_header_end),
        "download_block_operations_start": as_delta_or(stats.download_block_operations_start),
        "download_block_operations_end": as_delta_or(stats.load_data_start.and(stats.download_block_operations_end)),

        "load_data_start": as_delta_or(stats.load_data_start),
        "load_data_end": as_delta_or(stats.load_data_end),

        "apply_block_start": as_delta_or(stats.apply_block_start),
        "apply_block_end": as_delta_or(stats.apply_block_end),

        "store_result_start": as_delta_or(stats.store_result_start),
        "store_result_end": as_delta_or(stats.store_result_end),

        "send_start": as_delta_or(stats.head_send_start),
        "send_end": as_delta_or(first_send_end_time.cloned()),
        "last_send_end": as_delta_or(stats.head_send_end),

        "protocol_times": protocol_times,
    })
}

pub async fn dev_shell_automaton_stats_current_head_application(
    _: Request<Body>,
    _: Params,
    query: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    let level = query
        .get_str("level")
        .ok_or_else(|| anyhow::anyhow!("Missing mandatory query parameter `level`"))
        .and_then(|str| Ok(str.parse()?))?;
    let round = query.get_str("round").and_then(|str| str.parse().ok());

    let stats = dev_services::get_shell_automaton_stats_current_head(level, round, &env).await?;
    let base_time = stats
        .iter()
        .min_by_key(|(_, v)| v.receive_timestamp)
        .map(|(_, v)| v.receive_timestamp)
        .unwrap_or_default();
    let result = stats
        .into_iter()
        .map(|(hash, stats)| application_stats(hash, stats, base_time))
        .collect::<Vec<_>>();

    make_json_response(&result)
}

#[derive(Debug, Clone, serde::Serialize)]
struct PeerStats {
    address: SocketAddr,
    node_id: Option<CryptoboxPublicKeyHash>,
    block_hash: BlockHash,
    received_time: u64,
    sent_start_time: u64,
    sent_end_time: u64,
    sent_time: u64,
    get_operations_recv_start_time: u64,
    get_operations_recv_end_time: u64,
    operations_send_start_time: u64,
    operations_send_end_time: u64,
}

fn fold_validation_pass(
    (prev, existing, num): (Option<u64>, i8, u8),
    (time, validation_pass): &(u64, i8),
) -> (Option<u64>, i8, u8) {
    if existing & (1 << validation_pass) == 0 {
        (Some(*time), existing | (1 << validation_pass), num + 1)
    } else {
        (prev, existing, num)
    }
}

#[cfg(test)]
mod fold_test {
    #[test]
    fn fold_validation_pass() {
        let folded = vec![]
            .iter()
            .fold((None, 0, 0), super::fold_validation_pass);
        assert_eq!(folded, (None, 0, 0));

        let folded = vec![(100, 1)]
            .iter()
            .fold((None, 0, 0), super::fold_validation_pass);
        assert_eq!(folded, (Some(100), 2, 1));

        let folded = vec![(100, 1), (200, 2)]
            .iter()
            .fold((None, 0, 0), super::fold_validation_pass);
        assert_eq!(folded, (Some(200), 6, 2));

        let folded = vec![(100, 1), (200, 1)]
            .iter()
            .fold((None, 0, 0), super::fold_validation_pass);
        assert_eq!(folded, (Some(100), 2, 1));
    }
}

fn peers_stats(
    hash: &BlockHash,
    address: SocketAddr,
    stats: &BlockPeerStats,
    base_time: u64,
) -> serde_json::Value {
    let as_delta = |time: u64| time.saturating_sub(base_time);
    let as_delta_or = |time: Option<u64>| time.map(as_delta);

    let head_send_start = as_delta_or(stats.head_send_start.first().cloned());
    let head_send_end = as_delta_or(stats.head_send_end.first().cloned());
    let get_ops_recv_start = as_delta_or(stats.get_ops_recv.first().map(|(t, _)| *t));
    let (get_ops_recv_end, _, get_ops_recv_num) = stats
        .get_ops_recv
        .iter()
        .fold((None, 0, 0), fold_validation_pass);
    let ops_send_start = as_delta_or(stats.ops_send_start.first().map(|(t, _)| *t));
    let (ops_send_end, _, ops_send_num) = stats
        .ops_send_end
        .iter()
        .fold((None, 0, 0), fold_validation_pass);

    serde_json::json!({
        "address": address,
        "node_id": stats.node_id,
        "block_hash": hash,
        "received_time": as_delta_or(stats.head_recv.first().cloned()),
        "sent_start_time": head_send_start,
        "sent_time": head_send_end,
        "sent_end_time": head_send_end,
        "get_operations_recv_start_time": get_ops_recv_start,
        "get_operations_recv_end_time": as_delta_or(get_ops_recv_end),
        "get_operations_recv_num": get_ops_recv_num,
        "operations_send_start_time": ops_send_start,
        "operations_send_end_time": as_delta_or(ops_send_end),
        "operations_send_num": ops_send_num,
        "raw": stats,
    })
}

pub async fn dev_shell_automaton_stats_current_head_peers(
    _: Request<Body>,
    _: Params,
    query: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    let level = query
        .get_str("level")
        .ok_or_else(|| anyhow::anyhow!("Missing mandatory query parameter `level`"))
        .and_then(|str| Ok(str.parse()?))?;
    let round = query.get_str("round").and_then(|str| str.parse().ok());

    let stats = dev_services::get_shell_automaton_stats_current_head(level, round, &env).await?;
    let base_time = stats
        .iter()
        .min_by_key(|(_, v)| v.receive_timestamp)
        .map(|(_, v)| v.receive_timestamp)
        .unwrap_or_default();
    let result = stats
        .into_iter()
        .flat_map(|(hash, stats)| {
            stats
                .peers
                .iter()
                .map(|(address, stats)| peers_stats(&hash, *address, stats, base_time))
                .collect::<Vec<_>>()
                .into_iter()
        })
        .collect::<Vec<_>>();

    make_json_response(&result)
}

pub async fn dev_shell_automaton_endrosement_stats(
    _: Request<Body>,
    _: Params,
    _: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    make_json_response(&dev_services::get_shell_automaton_endrosement_stats(&env).await?)
}
// best_remote_level

pub async fn best_remote_level(
    _: Request<Body>,
    _: Params,
    _: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    make_json_response(&dev_services::get_best_remote_level(&env).await?)
}

pub async fn dev_cycle_rewards(
    _: Request<Body>,
    params: Params,
    query: Query,
    env: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    let chain_id_param = MAIN_CHAIN_ID;
    let chain_id = parse_chain_id(chain_id_param, &env)?;
    let cycle_num = required_param!(params, "cycle_num")?.parse()?;
    let delegate = query.get_str("delegate").map(|v| v.to_string());
    let commission: Option<i32> = query.get_parsed("comission")?;
    let exclude_accusation_rewards = query.contains_key("exclude_accusation_rewards");

    let filter = CycleRewardsFilter::new(delegate, commission, exclude_accusation_rewards);

    make_json_response(
        &rewards_services::get_cycle_rewards_distribution(&chain_id, &env, cycle_num, filter)
            .await?,
    )
}
