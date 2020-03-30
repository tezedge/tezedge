// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use chrono::prelude::*;
use hyper::{Body, Request, Response, StatusCode};
use slog::{Logger, warn};

use crypto::hash::HashType;
use shell::shell_channel::BlockApplied;

use crate::{
    encoding::{base_types::*, monitor::BootstrapInfo}, make_json_response,
    ServiceResult,
    ts_to_rfc3339,
};
use crate::helpers::RpcResponseData;
use crate::rpc_actor::RpcCollectedStateRef;
use crate::server::{HResult, Params, Query, RpcServiceEnvironment};
use crate::server::service;

/// Helper function for generating current TimeStamp
#[allow(dead_code)]
fn timestamp() -> TimeStamp {
    TimeStamp::Integral(Utc::now().timestamp())
}

/// Generate 404 response
fn not_found() -> ServiceResult {
    Ok(Response::builder()
        .status(StatusCode::from_u16(404)?)
        .body(Body::from("not found"))?)
}

/// Generate empty response
pub fn empty() -> ServiceResult {
    Ok(Response::builder()
        .status(StatusCode::from_u16(204)?)
        .body(Body::empty())?)
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
    let chain_id = params.get_str("chain_id").unwrap();
    let block_id = params.get_str("block_id").unwrap();

    result_to_json_response(service::get_context_constants(chain_id, block_id, None, env.persistent_storage().context_storage(), env.persistent_storage(), env.state()), env.log())
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
    match service::check_and_get_baking_rights(chain_id, block_id, level, delegate, cycle, max_priority, has_all, env.persistent_storage().context_storage(), env.persistent_storage(), env.state()) {
        Ok(Some(RpcResponseData::BakingRights(res))) => result_to_json_response(Ok(Some(res)), env.log()),
        // Ok(Some(RpcResponseData::ErrorMsg(res))) => result_to_json_response(Ok(Some(res)), &log),
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
    match service::check_and_get_endorsing_rights(chain_id, block_id, level, delegate, cycle, has_all, env.persistent_storage().context_storage(), env.persistent_storage(), env.state()) {
        Ok(Some(RpcResponseData::EndorsingRights(res))) => result_to_json_response(Ok(Some(res)), env.log()),
        // Ok(Some(RpcResponseData::ErrorMsg(res))) => result_to_json_response(Ok(Some(res)), &log),
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

    result_to_json_response(service::get_votes_listings(chain_id, block_id, env.persistent_storage(), env.persistent_storage().context_storage(), env.state()), env.log())
}

pub async fn p2p_messages(_: Request<Body>, params: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let start = params.get_str("offset").unwrap();
    let end = params.get_str("count").unwrap();

    result_to_json_response(service::retrieve_p2p_messages(start, end, env.persistent_storage()), env.log())
}

pub async fn  p2p_host_messages(_: Request<Body>, params: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let start = params.get_str("offset").unwrap();
    let end = params.get_str("count").unwrap();
    let host = params.get_str("host").unwrap();

    result_to_json_response(service::retrieve_host_p2p_messages(start, end, host, env.persistent_storage()), env.log())
}

pub async fn dev_blocks(_: Request<Body>, _: Params, query: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let from_block_id = unwrap_block_hash(query.get_str("from_block_id"), env.state(), env.genesis_hash());
    let limit = query.get_usize("limit").unwrap_or(50);
    let every_nth_level = match query.get_str("every_nth") {
        Some("cycle") => Some(4096),
        Some("voting-period") => Some(4096 * 8),
        _ => None
    };
    result_to_json_response(service::get_blocks(every_nth_level, &from_block_id, limit, env.persistent_storage(), env.state()), env.log())
}

pub async fn dev_block_actions(_: Request<Body>, params: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let block_id = params.get_str("block_id").unwrap();
    result_to_json_response(service::get_block_actions(block_id, env.persistent_storage(), env.state()), env.log())
}

pub async fn dev_contract_actions(_: Request<Body>, params: Params, query: Query, env: RpcServiceEnvironment) -> ServiceResult {
    let contract_id = params.get_str("contract_id").unwrap();
    let from_id = query.get_u64("from_id");
    let limit = query.get_usize("limit").unwrap_or(50);
    result_to_json_response(service::get_contract_actions(contract_id, from_id, limit, env.persistent_storage()), env.log())
}

pub async fn dev_context(_: Request<Body>, params: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    // TODO: Add parameter checks
    let context_level = params.get_str("id").unwrap();
    result_to_json_response(service::get_context(context_level, env.persistent_storage().context_storage()), env.log())
}

pub async fn dev_stats_memory(_: Request<Body>, _: Params, _: Query, env: RpcServiceEnvironment) -> ServiceResult {
    match service::get_stats_memory() {
        Ok(resp) => make_json_response(&resp),
        Err(e) => {
            warn!(env.log(), "GetStatsMemory: {}", e);
            empty()
        }
    }
}

/// Returns result as a JSON response.
fn result_to_json_response<T: serde::Serialize>(res: Result<T, failure::Error>, log: &Logger) -> ServiceResult {
    match res {
        Ok(t) => make_json_response(&t),
        Err(err) => {
            warn!(log, "Failed to execute RPC function"; "reason" => format!("{:?}", err));
            empty()
        }
    }
}

/// Returns optional result as a JSON response.
fn result_option_to_json_response<T: serde::Serialize>(res: Result<Option<T>, failure::Error>, log: &Logger) -> ServiceResult {
    match res {
        Ok(opt) => match opt {
            Some(t) => make_json_response(&t),
            None => not_found()
        }
        Err(err) => {
            warn!(log, "Failed to execute RPC function"; "reason" => format!("{:?}", err));
            empty()
        }
    }
}

/// Unwraps a block hash or provides alternative block hash.
/// Alternatives are: genesis block or current head
fn unwrap_block_hash(block_id: Option<&str>, state: &RpcCollectedStateRef, genesis_hash: &str) -> String {
    block_id.map(String::from).unwrap_or_else(|| {
        let state = state.read().unwrap();
        state.current_head().as_ref()
            .map(|current_head| HashType::BlockHash.bytes_to_string(&current_head.header().hash))
            .unwrap_or(genesis_hash.to_string())
    })
}


pub trait HasSingleValue {
    fn get_str(&self, key: &str) -> Option<&str>;

    fn get_u64(&self, key: &str) -> Option<u64> {
        self.get_str(key).and_then(|value| value.parse::<u64>().ok())
    }

    fn get_usize(&self, key: &str) -> Option<usize> {
        self.get_str(key).and_then(|value| value.parse::<usize>().ok())
    }

    fn contains_key(&self, key: &str) -> bool {
        self.get_str(key).is_some()
    }
}

impl HasSingleValue for Params {
    fn get_str(&self, key: &str) -> Option<&str> {
        self.iter().find_map(|(k, v)| {
            if k == key {
                Some(v.as_str())
            } else {
                None
            }
        })
    }
}

impl HasSingleValue for Query {
    fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key).map(|values| values.iter().next().map(String::as_str)).flatten()
    }
}


