// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use hyper::{Body, Request};
use slog::warn;

use crate::{
    result_to_json_response,
    ServiceResult,
    services,
};
use crate::helpers::create_rpc_request;
use crate::server::{HasSingleValue, Params, Query, RpcServiceEnvironment};
use crate::services::base_services;

pub async fn context_constants(_: Request<Body>, params: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let block_id = params.get_str("block_id").unwrap();

    result_to_json_response(base_services::get_context_constants_just_for_rpc(block_id, None, env.persistent_storage(), env.state()), env.log())
}

pub async fn cycle(_: Request<Body>, params: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let block_id = params.get_str("block_id").unwrap();
    let cycle_id = params.get_str("cycle_id").unwrap();
    result_to_json_response(services::protocol::get_cycle_from_context_as_json(block_id, cycle_id, env.persistent_storage(), env.tezedge_context(), env.state()), env.log())
}

pub async fn baking_rights(_: Request<Body>, params: Params, query: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let chain_id = params.get_str("chain_id").unwrap();
    let block_id = params.get_str("block_id").unwrap();
    let max_priority = query.get_str("max_priority");
    let level = query.get_str("level");
    let delegate = query.get_str("delegate");
    let cycle = query.get_str("cycle");
    let has_all = query.contains_key("all");

    // list -> context, persistent, state odizolovat
    match services::protocol::check_and_get_baking_rights(chain_id, block_id, level, delegate, cycle, max_priority, has_all, env.persistent_storage(), env.state()) {
        Ok(Some(rights)) => result_to_json_response(Ok(Some(rights)), env.log()),
        Err(e) => { //pass error to response parser
            let res: Result<Option<String>, failure::Error> = Err(e);
            result_to_json_response(res, env.log())
        }
        _ => { //ignore other options from enum
            warn!(env.log(), "Wrong RpcResponseData format");
            let res: Result<Option<String>, failure::Error> = Ok(None);
            result_to_json_response(res, env.log())
        }
    }
}

pub async fn endorsing_rights(_: Request<Body>, params: Params, query: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let chain_id = params.get_str("chain_id").unwrap();
    let block_id = params.get_str("block_id").unwrap();
    let level = query.get_str("level");
    let cycle = query.get_str("cycle");
    let delegate = query.get_str("delegate");
    let has_all = query.contains_key("all");

    // get RPC response and unpack it from RpcResponseData enum
    match services::protocol::check_and_get_endorsing_rights(chain_id, block_id, level, delegate, cycle, has_all, env.persistent_storage(), env.state()) {
        Ok(Some(rights)) => result_to_json_response(Ok(Some(rights)), env.log()),
        Err(e) => { //pass error to response parser
            let res: Result<Option<String>, failure::Error> = Err(e);
            result_to_json_response(res, env.log())
        }
        _ => { //ignore other options from enum
            warn!(env.log(), "Wrong RpcResponseData format");
            let res: Result<Option<String>, failure::Error> = Ok(None);
            result_to_json_response(res, env.log())
        }
    }
}

pub async fn votes_listings(_: Request<Body>, params: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let chain_id = params.get_str("chain_id").unwrap();
    let block_id = params.get_str("block_id").unwrap();

    result_to_json_response(services::protocol::get_votes_listings(chain_id, block_id, env.persistent_storage(), env.state()), env.log())
}

pub async fn get_contract_counter(_: Request<Body>, params: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let _chain_id = params.get_str("chain_id").unwrap();
    let block_id = params.get_str("block_id").unwrap();
    let pkh = params.get_str("pkh").unwrap();

    result_to_json_response(
        services::protocol::proto_get_contract_counter(_chain_id, block_id, pkh, env.persistent_storage(), env.state()),
        env.log(),
    )
}

pub async fn get_contract_manager_key(_: Request<Body>, params: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let _chain_id = params.get_str("chain_id").unwrap();
    let block_id = params.get_str("block_id").unwrap();
    let pkh = params.get_str("pkh").unwrap();

    result_to_json_response(
        services::protocol::proto_get_contract_manager_key(_chain_id, block_id, pkh, env.persistent_storage(), env.state()),
        env.log(),
    )
}

pub async fn call_protocol_rpc(req: Request<Body>, params: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let chain_param = params.get_str("chain_id").unwrap();
    let block_param = params.get_str("block_id").unwrap();

    let json_request = create_rpc_request(req).await?;

    result_to_json_response(
        services::protocol::call_protocol_rpc(chain_param, block_param, json_request, &env),
        env.log(),
    )
}
