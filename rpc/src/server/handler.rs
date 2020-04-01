// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use chrono::prelude::*;
use hyper::{Body, Request};
use slog::warn;

use crypto::hash::HashType;
use shell::shell_channel::BlockApplied;
use tezos_messages::ts_to_rfc3339;

use crate::{
    empty,
    encoding::{
        base_types::*,
        monitor::BootstrapInfo
    },
    make_json_response,
    result_option_to_json_response,
    result_to_json_response,
    ServiceResult,
    services
};
use crate::server::{HasSingleValue, HResult, Params, Query, RpcServiceEnvironment};
use crate::server::service;

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
        let current_head = service::get_full_current_head(env.state());
        if let Ok(Some(_current_head)) = current_head {
            // TODO: implement
            empty()
        } else {
            empty()
        }
    } else {
        empty()
    }
}

pub async fn chains_block_id(_: Request<Body>, params: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let chain_id = params.get_str("chain_id").unwrap();
    let block_id = params.get_str("block_id").unwrap();

    use crate::encoding::chain::BlockInfo;
    if chain_id == "main" {
        if block_id == "head" {
            result_option_to_json_response(service::get_full_current_head(env.state()).map(|res| res.map(BlockInfo::from)), env.log())
        } else {
            result_option_to_json_response(service::get_full_block(block_id, env.persistent_storage(), env.state()).map(|res| res.map(BlockInfo::from)), env.log())
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
            result_option_to_json_response(service::get_current_head_header(env.state()).map(|res| res), env.log())
        } else {
            result_option_to_json_response(service::get_block_header(block_id, env.persistent_storage(), env.state()).map(|res| res), env.log())
        }
    } else {
        empty()
    }
}

pub async fn context_constants(_: Request<Body>, params: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let block_id = params.get_str("block_id").unwrap();

    result_to_json_response(service::get_context_constants_just_for_rpc(block_id, None, env.persistent_storage().context_storage(), env.persistent_storage(), env.state()), env.log())
}

pub async fn context_cycle(_: Request<Body>, params: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let block_id = params.get_str("block_id").unwrap();

    result_to_json_response(service::get_cycle_from_context(block_id, env.persistent_storage().context_storage()), env.log())
}

pub async fn rolls_owner_current(_: Request<Body>, params: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let block_id = params.get_str("block_id").unwrap();
    result_to_json_response(service::get_rolls_owner_current_from_context(block_id, env.persistent_storage().context_storage()), env.log())
}

pub async fn cycle(_: Request<Body>, params: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let block_id = params.get_str("block_id").unwrap();
    let cycle_id = params.get_str("cycle_id").unwrap();
    result_to_json_response(service::get_cycle_from_context_as_json(block_id, cycle_id, env.persistent_storage().context_storage()), env.log())
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

pub async fn operations(_: Request<Body>, params: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let chain_id = params.get_str("chain_id").unwrap();
    let block_id = params.get_str("block_id").unwrap();

    result_to_json_response(services::protocol::get_operations_by_protocol(chain_id, block_id, env.persistent_storage(), env.persistent_storage().context_storage(), env.state()), env.log())
}
