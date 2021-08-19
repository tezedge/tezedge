// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use failure::format_err;
use hyper::{Body, Request};
use slog::warn;

use crate::helpers::{parse_block_hash, parse_chain_id, RpcServiceError, MAIN_CHAIN_ID};
use crate::result_option_to_json_response;
use crate::server::{HasSingleValue, Params, Query, RpcServiceEnvironment};
use crate::services::{context, dev_services};
use crate::{empty, make_json_response, required_param, result_to_json_response, ServiceResult};
use std::sync::Arc;

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
            match state.current_head() {
                Some(current_head) => current_head.hash.clone(),
                None => env.main_chain_genesis_hash().clone(),
            }
        }
    };

    // get cycle length
    let cycle_length = dev_services::get_cycle_length_for_block(&from_block_id, &env, env.log())
        .map_err(|e| format_err!("Failed to get constants, reason: {}", e))?;
    let every_nth_level = match query.get_str("every_nth") {
        Some("cycle") => Some(cycle_length),
        Some("voting-period") => Some(cycle_length * 8),
        _ => None,
    };
    let limit = query.get_usize("limit").unwrap_or(50);

    result_to_json_response(
        dev_services::get_blocks(chain_id, from_block_id, every_nth_level, limit, &env),
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
    let db_path = env.context_stats_db_path.as_ref();

    result_to_json_response(
        context::make_context_stats(db_path, context_name),
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

/// Get the version string
pub async fn dev_version(
    _: Request<Body>,
    _: Params,
    _: Query,
    _: Arc<RpcServiceEnvironment>,
) -> ServiceResult {
    make_json_response(&dev_services::get_dev_version())
}
