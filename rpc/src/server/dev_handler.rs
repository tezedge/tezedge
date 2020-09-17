// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use hyper::{Body, Request};
use slog::warn;

use crate::{empty, make_json_response, result_to_json_response, ServiceResult, unwrap_block_hash};
use crate::server::{HasSingleValue, Params, Query, RpcServiceEnvironment};
use crate::services::base_services;

pub async fn dev_blocks(_: Request<Body>, _: Params, query: Query, env: RpcServiceEnvironment) -> ServiceResult {
    warn!(env.log(), "Getting dev_blocks");
    let from_block_id = unwrap_block_hash(query.get_str("from_block_id"), env.state(), env.genesis_hash());
    let limit = query.get_usize("limit").unwrap_or(50);
    let cycle_length = base_services::get_cycle_length_for_block(&from_block_id, env.persistent_storage(), env.state(), env.log())?;
    let every_nth_level = match query.get_str("every_nth") {
        Some("cycle") => Some(cycle_length),
        Some("voting-period") => Some(cycle_length * 8),
        _ => None
    };
    result_to_json_response(base_services::get_blocks(every_nth_level, &from_block_id, limit, env.persistent_storage(), env.state()), env.log())
}

#[allow(dead_code)]
pub async fn dev_block_actions(_: Request<Body>, params: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let block_id = params.get_str("block_hash").unwrap();
    result_to_json_response(base_services::get_block_actions(block_id, env.persistent_storage(), env.state()), env.log())
}

#[allow(dead_code)]
pub async fn dev_contract_actions(_: Request<Body>, params: Params, query: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let contract_id = params.get_str("contract_address").unwrap();
    let from_id = query.get_u64("from_id");
    let limit = query.get_usize("limit").unwrap_or(50);
    result_to_json_response(base_services::get_contract_actions(contract_id, from_id, limit, env.persistent_storage()), env.log())
}

pub async fn dev_action_cursor(_: Request<Body>, params: Params, query: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let cursor_id = query.get_u64("cursor_id");
    let limit = query.get_u64("limit").map(|limit| limit as usize);
    let action_types = query.get_str("action_types");
    result_to_json_response(if let Some(block_hash) = params.get_str("block_hash") {
        base_services::get_block_actions_cursor(block_hash, cursor_id, limit, action_types, env.persistent_storage(), env.state())
    } else if let Some(contract_address) = params.get_str("contract_address") {
        base_services::get_contract_actions_cursor(contract_address, cursor_id, limit, action_types, env.persistent_storage())
    } else {
        unreachable!()
    }, env.log())
}

pub async fn dev_context(_: Request<Body>, params: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    // TODO: Add parameter checks
    let context_level = params.get_str("id").unwrap();
    result_to_json_response(base_services::get_context(context_level, env.persistent_storage().context_storage()), env.log())
}

#[allow(dead_code)]
pub async fn dev_stats_storage(_: Request<Body>, _: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    result_to_json_response(
        crate::services::stats_services::compute_storage_stats(
            env.state(),
            env.genesis_hash(),
            env.persistent_storage()),
        env.log())
}

pub async fn dev_stats_memory(_: Request<Body>, _: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    match base_services::get_stats_memory() {
        Ok(resp) => make_json_response(&resp),
        Err(e) => {
            warn!(env.log(), "GetStatsMemory: {}", e);
            empty()
        }
    }
}
