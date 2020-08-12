// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use bytes::buf::BufExt;
use chrono::prelude::*;
use hyper::{Body, Request};
use slog::warn;

use crypto::hash::HashType;
use shell::shell_channel::BlockApplied;
use tezos_api::ffi::JsonRpcRequest;
use tezos_messages::ts_to_rfc3339;

use crate::{
    empty,
    encoding::{
        base_types::*,
        monitor::BootstrapInfo,
    },
    make_json_response,
    make_json_stream_response,
    result_option_to_json_response,
    result_to_json_response,
    ServiceResult,
    services,
};
use crate::server::{HasSingleValue, HResult, Params, Query, RpcServiceEnvironment};
use crate::services::base_services;

/// Helper function for generating current TimeStamp
#[allow(dead_code)]
fn timestamp() -> TimeStamp {
    TimeStamp::Integral(Utc::now().timestamp())
}

pub async fn bootstrapped(_: Request<Body>, _: Params, _: Query, env: RpcServiceEnvironment) -> HResult {
    let state_read = env.state().read().unwrap();

    let bootstrap_info = match state_read.current_head().as_ref() {
        Some(current_head) => {
            let current_head: BlockApplied = current_head.clone();
            let block = HashType::BlockHash.bytes_to_string(&current_head.header().hash);
            let timestamp = ts_to_rfc3339(current_head.header().header.timestamp());
            BootstrapInfo::new(block.into(), TimeStamp::Rfc(timestamp))
        }
        None => BootstrapInfo::new(String::new().into(), TimeStamp::Integral(0))
    };

    make_json_response(&bootstrap_info)
}

pub async fn commit_hash(_: Request<Body>, _: Params, _: Query, _: RpcServiceEnvironment) -> HResult {
    let resp = &UniString::from(env!("GIT_HASH"));
    make_json_response(&resp)
}

pub async fn active_chains(_: Request<Body>, _: Params, _: Query, _: RpcServiceEnvironment) -> HResult {
    empty()
}

pub async fn protocols(_: Request<Body>, _: Params, _: Query, _: RpcServiceEnvironment) -> HResult {
    empty()
}

pub async fn valid_blocks(_: Request<Body>, _: Params, _: Query, _: RpcServiceEnvironment) -> HResult {
    empty()
}

pub async fn head_chain(_: Request<Body>, params: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let chain_id = params.get_str("chain_id").unwrap();

    if chain_id == "main" {
        make_json_stream_response(base_services::get_current_head_monitor_header(env.state())?.unwrap())
    } else {
        // TODO: implement... 
        empty()
    }
}

pub async fn chains_block_id(_: Request<Body>, params: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let chain_id = params.get_str("chain_id").unwrap();
    let block_id = params.get_str("block_id").unwrap();

    use crate::encoding::chain::BlockInfo;
    if chain_id == "main" {
        if block_id == "head" {
            result_option_to_json_response(base_services::get_full_current_head(env.state()).map(|res| res.map(BlockInfo::from)), env.log())
        } else {
            result_option_to_json_response(base_services::get_full_block(block_id, env.persistent_storage(), env.state()).map(|res| res.map(BlockInfo::from)), env.log())
        }
    } else {
        empty()
    }
}

pub async fn chains_block_id_header(_: Request<Body>, params: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let chain_id = params.get_str("chain_id").unwrap();
    let block_id = params.get_str("block_id").unwrap();

    if chain_id == "main" {
        if block_id == "head" {
            result_option_to_json_response(base_services::get_current_head_header(env.state()).map(|res| res), env.log())
        } else {
            result_option_to_json_response(base_services::get_block_header(block_id, env.persistent_storage(), env.state()).map(|res| res), env.log())
        }
    } else {
        empty()
    }
}

pub async fn chains_block_id_header_shell(_: Request<Body>, params: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let chain_id = params.get_str("chain_id").unwrap();
    let block_id = params.get_str("block_id").unwrap();

    if chain_id == "main" {
        if block_id == "head" {
            result_option_to_json_response(base_services::get_current_head_shell_header(env.state()).map(|res| res), env.log())
        } else {
            result_option_to_json_response(base_services::get_block_shell_header(block_id, env.persistent_storage(), env.state()).map(|res| res), env.log())
        }
    } else {
        empty()
    }
}

pub async fn context_constants(_: Request<Body>, params: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let block_id = params.get_str("block_id").unwrap();

    result_to_json_response(base_services::get_context_constants_just_for_rpc(block_id, None, env.persistent_storage().context_storage(), env.persistent_storage(), env.state()), env.log())
}

pub async fn context_cycle(_: Request<Body>, params: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let block_id = params.get_str("block_id").unwrap();

    result_to_json_response(base_services::get_cycle_from_context(block_id, env.persistent_storage().context_storage(), env.persistent_storage()), env.log())
}

pub async fn rolls_owner_current(_: Request<Body>, params: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let block_id = params.get_str("block_id").unwrap();
    result_to_json_response(base_services::get_rolls_owner_current_from_context(block_id, env.persistent_storage().context_storage(), env.persistent_storage()), env.log())
}

pub async fn cycle(_: Request<Body>, params: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let block_id = params.get_str("block_id").unwrap();
    let cycle_id = params.get_str("cycle_id").unwrap();
    result_to_json_response(base_services::get_cycle_from_context_as_json(block_id, cycle_id, env.persistent_storage().context_storage(), env.persistent_storage()), env.log())
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
    match services::protocol::check_and_get_baking_rights(chain_id, block_id, level, delegate, cycle, max_priority, has_all, env.persistent_storage().context_storage(), env.persistent_storage(), env.state()) {
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
    match services::protocol::check_and_get_endorsing_rights(chain_id, block_id, level, delegate, cycle, has_all, env.persistent_storage().context_storage(), env.persistent_storage(), env.state()) {
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

    result_to_json_response(services::protocol::get_votes_listings(chain_id, block_id, env.persistent_storage(), env.persistent_storage().context_storage(), env.state()), env.log())
}

pub async fn mempool_pending_operations(_: Request<Body>, params: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let chain_id = params.get_str("chain_id").unwrap();

    if chain_id == "main" {
        result_to_json_response(
            services::mempool_services::get_pending_operations(env.persistent_storage(), env.state(), env.log()),
            env.log(),
        )
    } else {
        unimplemented!("not implemented yet")
    }
}

pub async fn inject_operation(req: Request<Body>, _: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let operation_data_raw = hyper::body::aggregate(req).await?;
    let operation_data: String = serde_json::from_reader(&mut operation_data_raw.reader())?;

    let shell_channel = env.shell_channel();

    result_to_json_response(
        services::mempool_services::inject_operation(&operation_data, env.persistent_storage(), env.state(), shell_channel.clone(), env.log()),
        env.log(),
    )
}

pub async fn inject_block(req: Request<Body>, _: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let body = hyper::body::to_bytes(req.into_body()).await?;
    let body = String::from_utf8(body.to_vec())?;

    let shell_channel = env.shell_channel();

    result_to_json_response(
        services::mempool_services::inject_block(&body, &env, shell_channel.clone()),
        env.log(),
    )
}

pub async fn get_block_protocols(_: Request<Body>, params: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let _chain_id = params.get_str("chain_id").unwrap();
    let block_id = params.get_str("block_id").unwrap();


    result_to_json_response(
        base_services::get_block_protocols(block_id, env.persistent_storage(), env.state()),
        env.log(),
    )
}

pub async fn get_block_hash(_: Request<Body>, params: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let _chain_id = params.get_str("chain_id").unwrap();
    let block_id = params.get_str("block_id").unwrap();

    result_to_json_response(
        base_services::get_block_hash(block_id, env.persistent_storage(), env.state()),
        env.log(),
    )
}

pub async fn get_chain_id(_: Request<Body>, params: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    // this chain_id (e.g. main) reporesents the "alias" for the actial base58 encoded id (e.g. NetXdQprcVkpaWU)
    let _chain_id = params.get_str("chain_id").unwrap();

    result_to_json_response(
        base_services::get_chain_id(env.state()),
        env.log(),
    )
}

pub async fn get_contract_counter(_: Request<Body>, params: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let _chain_id = params.get_str("chain_id").unwrap();
    let block_id = params.get_str("block_id").unwrap();
    let pkh = params.get_str("pkh").unwrap();

    result_to_json_response(
        services::protocol::proto_get_contract_counter(_chain_id, block_id, pkh, env.persistent_storage().context_storage(), env.persistent_storage(), env.state()),
        env.log(),
    )
}

pub async fn get_contract_manager_key(_: Request<Body>, params: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let _chain_id = params.get_str("chain_id").unwrap();
    let block_id = params.get_str("block_id").unwrap();
    let pkh = params.get_str("pkh").unwrap();

    result_to_json_response(
        services::protocol::proto_get_contract_manager_key(_chain_id, block_id, pkh, env.persistent_storage().context_storage(), env.persistent_storage(), env.state()),
        env.log(),
    )
}

pub async fn get_block_operation_hashes(_: Request<Body>, params: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let _chain_id = params.get_str("chain_id").unwrap();
    let block_id = params.get_str("block_id").unwrap();


    result_to_json_response(
        base_services::get_block_operation_hashes(block_id, env.persistent_storage(), env.state()),
        env.log(),
    )
}

pub async fn run_operation(req: Request<Body>, params: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let chain_param = params.get_str("chain_id").unwrap();
    let block_param = params.get_str("block_id").unwrap();

    let json_request = create_ffi_json_request(req).await?;

    result_to_json_response(
        services::protocol::run_operation(chain_param, block_param, json_request, &env),
        env.log(),
    )
}

pub async fn current_level(req: Request<Body>, params: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let chain_param = params.get_str("chain_id").unwrap();
    let block_param = params.get_str("block_id").unwrap();

    let json_request = create_ffi_json_request(req).await?;

    result_to_json_response(
        services::protocol::current_level(chain_param, block_param, json_request, &env),
        env.log(),
    )
}

pub async fn minimal_valid_time(req: Request<Body>, params: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let chain_param = params.get_str("chain_id").unwrap();
    let block_param = params.get_str("block_id").unwrap();

    let json_request = create_ffi_json_request(req).await?;

    result_to_json_response(
        services::protocol::minimal_valid_time(chain_param, block_param, json_request, &env),
        env.log(),
    )
}

pub async fn live_blocks(_: Request<Body>, params: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let chain_param = params.get_str("chain_id").unwrap();
    let block_param = params.get_str("block_id").unwrap();

    result_to_json_response(
        services::protocol::live_blocks(chain_param, block_param, &env),
        env.log(),
    )
}

pub async fn preapply_operations(req: Request<Body>, params: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let chain_param = params.get_str("chain_id").unwrap();
    let block_param = params.get_str("block_id").unwrap();

    let json_request = create_ffi_json_request(req).await?;

    result_to_json_response(
        services::protocol::preapply_operations(chain_param, block_param, json_request, &env),
        env.log(),
    )
}

pub async fn preapply_block(req: Request<Body>, params: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let chain_param = params.get_str("chain_id").unwrap();
    let block_param = params.get_str("block_id").unwrap();

    let json_request = create_ffi_json_request(req).await?;

    result_to_json_response(
        services::protocol::preapply_block(chain_param, block_param, json_request, &env),
        env.log(),
    )
}

pub async fn node_version(_: Request<Body>, _: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    result_to_json_response(
        base_services::get_node_version(env.network_version()),
        env.log(),
    )
}

async fn create_ffi_json_request(req: Request<Body>) -> Result<JsonRpcRequest, failure::Error> {
    let context_path = req.uri().path_and_query().unwrap().as_str().to_string();
    let body = hyper::body::to_bytes(req.into_body()).await?;
    let body = String::from_utf8(body.to_vec())?;

    Ok(JsonRpcRequest {
        body,
        context_path,
    })
}