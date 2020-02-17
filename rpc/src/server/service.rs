// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;
use std::net::SocketAddr;

use chrono::prelude::*;
use futures::Future;
use hyper::{Body, Error, Method, Request, Response, Server, StatusCode, Uri};
use hyper::service::{make_service_fn, service_fn};
use path_tree::PathTree;
use riker::actors::ActorSystem;
use slog::{Logger, warn};

use crypto::hash::{BlockHash, HashType};
use lazy_static::lazy_static;
use shell::shell_channel::BlockApplied;
use storage::persistent::PersistentStorage;

use crate::{
    encoding::{base_types::*, monitor::BootstrapInfo}, make_json_response, rpc_actor::RpcServerRef,
    ServiceResult,
    ts_to_rfc3339,
};
use crate::rpc_actor::RpcCollectedStateRef;
use crate::helpers::RpcResponseData;
use crate::ContextList;


#[derive(Debug)]
enum Route {
    Bootstrapped,
    CommitHash,
    ActiveChains,
    Protocols,
    ValidBlocks,
    HeadChain,
    ChainsBlockId,
    ChainsBlockIdHeader,
    ContextConstants,
    ContextCycle,
    ContextRollsOwnerCurrent,
    ContextJsonCycle,
    ChainsBlockIdBakingRights,
    ChainsBlockIdEndorsingRights,
    // -------------------------- //
    DevGetBlocks,
    DevGetBlockActions,
    DevGetContext,
    DevGetContractActions,
    StatsMemory,
}

/// Server environment parameters
#[derive(Clone)]
pub(crate) struct RpcServiceEnvironment {
    sys: ActorSystem,
    actor: RpcServerRef,
    persistent_storage: PersistentStorage,
    genesis_hash: String,
    state: RpcCollectedStateRef,
    log: Logger,
}

impl RpcServiceEnvironment {
    pub fn new(sys: ActorSystem, actor: RpcServerRef, persistent_storage: &PersistentStorage, genesis_hash: &BlockHash, state: RpcCollectedStateRef, log: Logger) -> Self {
        Self { sys, actor, persistent_storage: persistent_storage.clone(), genesis_hash: HashType::BlockHash.bytes_to_string(genesis_hash), state, log }
    }
}

/// Spawn new HTTP server on given address interacting with specific actor system
pub(crate) fn spawn_server(addr: &SocketAddr, env: RpcServiceEnvironment) -> impl Future<Output=Result<(), Error>> {
    Server::bind(addr)
        .serve(make_service_fn(move |_| {
            let env = env.clone();
            async move {
                let env = env.clone();
                Ok::<_, Error>(service_fn(move |req| {
                    let env = env.clone();
                    async move {
                        router(req, env).await
                    }
                }))
            }
        }))
}

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
fn empty() -> ServiceResult {
    Ok(Response::builder()
        .status(StatusCode::from_u16(204)?)
        .body(Body::empty())?)
}

/// Helper for parsing URI queries.
/// Functions takes URI query in format `key1=val1&key1=val2&key2=val3`
/// and produces map `{ key1: [val1, val2], key2: [val3] }`
fn parse_query_string(query: &str) -> HashMap<&str, Vec<&str>> {
    let mut ret: HashMap<&str, Vec<&str>> = HashMap::new();
    for (key, value) in query.split('&').map(|x| {
        let mut parts = x.split('=');
        (parts.next().unwrap(), parts.next().unwrap_or(""))
    }) {
        if let Some(vals) = ret.get_mut(key) {
            vals.push(value);
        } else {
            ret.insert(key, vec![value]);
        }
    }
    ret
}

/// Gets a single value from parsed query.
#[inline]
fn find_query_value<'a, 'b>(query: &'a HashMap<&'a str, Vec<&'a str>>, key: &'b str) -> Option<&'a str> {
    query.get(key).and_then(|values| values.first().map(|v| *v))
}

/// Gets a single `String` value from parsed query.
#[inline]
fn find_query_value_as_string<'a, 'b>(query: &'a HashMap<&'a str, Vec<&'a str>>, key: &'b str) -> Option<String> {
    find_query_value(query, key).map(|value| value.to_string())
}

/// Gets a multiple `String` values from parsed query.
#[inline]
fn get_query_values_as_string<'a, 'b>(query: &'a HashMap<&'a str, Vec<&'a str>>, key: &'b str) -> Vec<String> {
    query.get(key).map(|values| values.iter().map(|value| value.to_string()).collect()).unwrap_or_else(|| Vec::new())
}

/// Gets a multiple `UniString` values from parsed query.
#[inline]
fn get_query_values_as_unistring<'a, 'b>(query: &'a HashMap<&'a str, Vec<&'a str>>, key: &'b str) -> Vec<UniString> {
    query.get(key).map(|values| values.iter().map(|&value| value.into()).collect()).unwrap_or_else(|| Vec::new())
}

/// Gets a single `usize` value from parsed query.
#[inline]
fn find_query_value_as_usize<'a, 'b>(query: &'a HashMap<&'a str, Vec<&'a str>>, key: &'b str) -> Option<usize> {
    find_query_value(query, key).and_then(|value| value.parse::<usize>().ok())
}

/// Gets a single `u64` value from parsed query.
#[inline]
fn find_query_value_as_u64<'a, 'b>(query: &'a HashMap<&'a str, Vec<&'a str>>, key: &'b str) -> Option<u64> {
    find_query_value(query, key).and_then(|value| value.parse::<u64>().ok())
}

/// Finds a parameter in a parameter array. This has complexity of O(n) but number of parameters
/// is fairly low (less than 4) so I'm fine with it.
#[inline]
fn find_param_value<'a, 'b>(params: &[(&'a str, &'a str)], key_to_find: &'b str) -> Option<&'a str> {
    params.iter().find_map(|&(key, value)| {
        if key == key_to_find {
            Some(value)
        } else {
            None
        }
    })
}

/// GET /monitor/bootstrapped endpoint handler
fn bootstrapped(state: RpcCollectedStateRef) -> ServiceResult {
    let state_read = state.read().unwrap();

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

/// GET /monitor/commit_hash endpoint handler
async fn commit_hash(_sys: ActorSystem, _actor: RpcServerRef) -> ServiceResult {
    let resp = &UniString::from(env!("GIT_HASH"));
    make_json_response(&resp)
}

async fn active_chains(_sys: ActorSystem, _actor: RpcServerRef) -> ServiceResult {
    empty()
}

async fn protocols(_sys: ActorSystem, _actor: RpcServerRef) -> ServiceResult {
    empty()
}

async fn valid_blocks(_sys: ActorSystem, _actor: RpcServerRef, _protocols: Vec<String>, _next_protocol: Vec<String>, _chain: Vec<UniString>) -> ServiceResult {
    empty()
}

fn head_chain(chain_id: &str, state: RpcCollectedStateRef) -> ServiceResult {
    if chain_id == "main" {
        let current_head = fns::get_full_current_head(state);
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

/// GET /chains/<chain_id>/blocks/<block_id> endpoint handler
fn chains_block_id(chain_id: &str, block_id: &str, persistent_storage: &PersistentStorage, state: RpcCollectedStateRef, log: &Logger) -> ServiceResult {
    use crate::encoding::chain::BlockInfo;
    if chain_id == "main" {
        if block_id == "head" {
            result_option_to_json_response(fns::get_full_current_head(state).map(|res| res.map(BlockInfo::from)), log)
        } else {
            result_option_to_json_response(fns::get_full_block(block_id, persistent_storage, state).map(|res| res.map(BlockInfo::from)), log)
        }
    } else {
        empty()
    }
}

/// GET /chains/<chain_id>/blocks/<block_id>/header endpoint handler
fn chains_block_id_header(chain_id: &str, block_id: &str, persistent_storage: &PersistentStorage, state: RpcCollectedStateRef, log: &Logger) -> ServiceResult {
    if chain_id == "main" {
        if block_id == "head" {
            result_option_to_json_response(fns::get_current_head_header(state).map(|res| res), log)
        } else {
            result_option_to_json_response(fns::get_block_header(block_id, persistent_storage, state).map(|res| res), log)
        }
    } else {
        empty()
    }
}

/// GET /stats/memory endpoint handler
async fn stats_memory(log: &Logger) -> ServiceResult {
    match fns::get_stats_memory() {
        Ok(resp) => make_json_response(&resp),
        Err(e) => {
            warn!(log, "GetStatsMemory: {}", e);
            empty()
        }
    }
}

/// GET /chains/<chain_id>/blocks/<block_id>/helpers/baking_rights endpoint handler
async fn baking_rights(chain_id: &str, block_id: &str, delegate: &Option<String>, level: &Option<String>, cycle: &Option<String>, max_priority: &Option<String>, has_all: bool, state: RpcCollectedStateRef, persistent_storage: &PersistentStorage, list: ContextList, log: &Logger) -> ServiceResult {
    // list -> context, persistent, state odizolovat
    match fns::check_and_get_baking_rights(chain_id, block_id, level, delegate, cycle, max_priority, has_all, list, persistent_storage, state) {
        Ok(Some(RpcResponseData::BakingRights(res))) => result_to_json_response(Ok(Some(res)), &log),
        // Ok(Some(RpcResponseData::ErrorMsg(res))) => result_to_json_response(Ok(Some(res)), &log),
        Err(e) => { //pass error to response parser
            let res: Result<Option<String>, failure::Error> = Err(e);
            result_to_json_response(res, &log)
        }
        _ => { //ignore other options from enum
            warn!(&log, "Wrong RpcResponseData format");
            let res: Result<Option<String>, failure::Error> = Ok(None);
            result_to_json_response(res, &log)
        }
    }
}

/// GET /chains/<chain_id>/blocks/<block_id>/helpers/endorsing_rights endpoint handler
async fn endorsing_rights(chain_id: &str, block_id: &str, delegate: &Option<String>, level: &Option<String>, cycle: &Option<String>, has_all: bool, state: RpcCollectedStateRef, persistent_storage: &PersistentStorage, list: ContextList, log: &Logger) -> ServiceResult {
    // get RPC response and unpack it from RpcResponseData enum
    match fns::check_and_get_endorsing_rights(chain_id, block_id, level, delegate, cycle, has_all, list, persistent_storage, state) {
        Ok(Some(RpcResponseData::EndorsingRights(res))) => result_to_json_response(Ok(Some(res)), &log),
        // Ok(Some(RpcResponseData::ErrorMsg(res))) => result_to_json_response(Ok(Some(res)), &log),
        Err(e) => { //pass error to response parser
            let res: Result<Option<String>, failure::Error> = Err(e);
            result_to_json_response(res, &log)
        }
        _ => { //ignore other options from enum
            warn!(&log, "Wrong RpcResponseData format");
            let res: Result<Option<String>, failure::Error> = Ok(None);
            result_to_json_response(res, &log)
        }
    }
}

lazy_static! {
    static ref ROUTES: PathTree<Route> = create_routes();
}

fn create_routes() -> PathTree<Route> {
    let mut routes = PathTree::new();
    routes.insert("/monitor/bootstrapped", Route::Bootstrapped);
    routes.insert("/monitor/commit_hash", Route::CommitHash);
    routes.insert("/monitor/active_chains", Route::ActiveChains);
    routes.insert("/monitor/protocols", Route::Protocols);
    routes.insert("/monitor/valid_blocks", Route::ValidBlocks);
    routes.insert("/monitor/heads/:chain_id", Route::HeadChain);
    routes.insert("/chains/:chain_id/blocks/:block_id", Route::ChainsBlockId);
    routes.insert("/chains/:chain_id/blocks/:block_id/header", Route::ChainsBlockIdHeader);
    routes.insert("/chains/:chain_id/blocks/:block_id/context/constants", Route::ContextConstants);
    routes.insert("/chains/:chain_id/blocks/:block_id/context/raw/bytes/cycle", Route::ContextCycle);
    routes.insert("/chains/:chain_id/blocks/:block_id/context/raw/bytes/rolls/owner/current", Route::ContextRollsOwnerCurrent);
    routes.insert("/chains/:chain_id/blocks/:block_id/context/raw/json/cycle/:cycle_id", Route::ContextJsonCycle);
    routes.insert("/chains/:chain_id/blocks/:block_id/helpers/baking_rights", Route::ChainsBlockIdBakingRights);
    routes.insert("/chains/:chain_id/blocks/:block_id/helpers/endorsing_rights", Route::ChainsBlockIdEndorsingRights);
    routes.insert("/dev/chains/main/blocks", Route::DevGetBlocks);
    routes.insert("/dev/chains/main/blocks/:block_id/actions", Route::DevGetBlockActions);
    routes.insert("/dev/chains/main/actions/contracts/:contract_id", Route::DevGetContractActions);
    routes.insert("/dev/context/:id", Route::DevGetContext);
    routes.insert("/stats/memory", Route::StatsMemory);
    routes
}

/// Simple endpoint routing handler
async fn router(req: Request<Body>, env: RpcServiceEnvironment) -> ServiceResult {
    let RpcServiceEnvironment { sys, actor, persistent_storage, log, genesis_hash, state } = env;
    let context_storage = persistent_storage.context_storage();

    match (req.method(), find_route(req.uri())) {
        (&Method::GET, Some((Route::Bootstrapped, _, _))) => bootstrapped(state),
        (&Method::GET, Some((Route::CommitHash, _, _))) => commit_hash(sys, actor).await,
        (&Method::GET, Some((Route::ActiveChains, _, _))) => active_chains(sys, actor).await,
        (&Method::GET, Some((Route::Protocols, _, _))) => protocols(sys, actor).await,
        (&Method::GET, Some((Route::StatsMemory, _, _))) => stats_memory(&log).await,
        (&Method::GET, Some((Route::ValidBlocks, _, query))) => {
            let protocol = get_query_values_as_string(&query, "protocol");
            let next_protocol = get_query_values_as_string(&query, "next_protocol");
            let chain = get_query_values_as_unistring(&query, "chain");
            valid_blocks(sys, actor, protocol, next_protocol, chain).await
        }
        (&Method::GET, Some((Route::ContextConstants, params, _))) => {
            let chain_id = find_param_value(&params, "chain_id").unwrap();
            let block_id = find_param_value(&params, "block_id").unwrap();
            result_to_json_response(fns::get_context_constants(chain_id, block_id, None, context_storage, &persistent_storage), &log)
        }
        (&Method::GET, Some((Route::HeadChain, params, _))) => {
            let chain_id = find_param_value(&params, "chain_id").unwrap();
            head_chain(chain_id, state)
        }
        (&Method::GET, Some((Route::ChainsBlockId, params, _))) => {
            let chain_id = find_param_value(&params, "chain_id").unwrap();
            let block_id = find_param_value(&params, "block_id").unwrap();
            chains_block_id(chain_id, block_id, &persistent_storage, state, &log)
        }
        (&Method::GET, Some((Route::ChainsBlockIdHeader, params, _))) => {
            let chain_id = find_param_value(&params, "chain_id").unwrap();
            let block_id = find_param_value(&params, "block_id").unwrap();
            chains_block_id_header(chain_id, block_id, &persistent_storage, state, &log)
        }
        (&Method::GET, Some((Route::DevGetBlocks, _, query))) => {
            let from_block_id = unwrap_block_hash(find_query_value_as_string(&query, "from_block_id"), state.clone(), genesis_hash);
            let limit = find_query_value_as_usize(&query, "limit").unwrap_or(50);
            let every_nth_level = match find_query_value_as_string(&query, "every_nth").as_ref().map(|x| x.as_str()) {
                Some("cycle") => Some(4096),
                Some("voting-period") => Some(4096 * 8),
                _ => None
            };
            result_to_json_response(fns::get_blocks(every_nth_level, &from_block_id, limit, &persistent_storage, state), &log)
        }
        (&Method::GET, Some((Route::DevGetBlockActions, params, _))) => {
            let block_id = find_param_value(&params, "block_id").unwrap();
            result_to_json_response(fns::get_block_actions(block_id, &persistent_storage), &log)
        }
        (&Method::GET, Some((Route::DevGetContractActions, params, query))) => {
            let contract_id = find_param_value(&params, "contract_id").unwrap();
            let from_id = find_query_value_as_u64(&query, "from_id");
            let limit = find_query_value_as_usize(&query, "limit").unwrap_or(50);
            result_to_json_response(fns::get_contract_actions(contract_id, from_id, limit, &persistent_storage), &log)
        }
        (&Method::GET, Some((Route::DevGetContext, params, _))) => {
            // TODO: Add parameter checks
            let context_level = find_param_value(&params, "id").unwrap();
            result_to_json_response(fns::get_context(context_level, context_storage), &log)
        }
        (&Method::GET, Some((Route::ContextCycle, params, _))) => {
            let block_id = find_param_value(&params, "block_id").unwrap();
            result_to_json_response(fns::get_cycle_from_context(block_id, context_storage), &log)
        }
        (&Method::GET, Some((Route::ContextRollsOwnerCurrent, params, _))) => {
            let block_id = find_param_value(&params, "block_id").unwrap();
            result_to_json_response(fns::get_rolls_owner_current_from_context(block_id, context_storage), &log)
        }
        (&Method::GET, Some((Route::ContextJsonCycle, params, _))) => {
            let block_id = find_param_value(&params, "block_id").unwrap();
            let cycle_id = find_param_value(&params, "cycle_id").unwrap();
            result_to_json_response(fns::get_cycle_from_context_as_json(block_id, cycle_id, context_storage), &log)
        }
        (&Method::GET, Some((Route::ChainsBlockIdEndorsingRights, params, query))) => {
            let chain_id = find_param_value(&params, "chain_id").unwrap();
            let block_id = find_param_value(&params, "block_id").unwrap();
            let level = find_query_value_as_string(&query, "level");
            let cycle = find_query_value_as_string(&query, "cycle");
            let delegate = find_query_value_as_string(&query, "delegate");
            let has_all = query.contains_key("all");

            endorsing_rights(chain_id, block_id, &delegate, &level, &cycle, has_all, state, &persistent_storage, context_storage, &log).await
        }
        (&Method::GET, Some((Route::ChainsBlockIdBakingRights, params, query))) => {
            let chain_id = find_param_value(&params, "chain_id").unwrap();
            let block_id = find_param_value(&params, "block_id").unwrap();
            let max_priority = find_query_value_as_string(&query, "max_priority");
            let level = find_query_value_as_string(&query, "level");
            let delegate = find_query_value_as_string(&query, "delegate");
            let cycle = find_query_value_as_string(&query, "cycle");
            let has_all = query.contains_key("all");

            baking_rights(chain_id, block_id, &delegate, &level, &cycle, &max_priority, has_all, state, &persistent_storage, context_storage, &log).await
        }
        _ => not_found()
    }
}

/// Find route and return a tuple with the following items:
/// * route type
/// * path parameters
/// * query parameters
#[inline]
fn find_route(uri: &Uri) -> Option<(&Route, Vec<(&str, &str)>, HashMap<&str, Vec<&str>>)> {
    ROUTES.find(uri.path()).map(|route| (route.0, route.1, uri.query().map(parse_query_string).unwrap_or_else(|| HashMap::new())))
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
fn unwrap_block_hash(block_id: Option<String>, state: RpcCollectedStateRef, genesis_hash: String) -> String {
    block_id.unwrap_or_else(|| {
        let state = state.read().unwrap();
        state.current_head().as_ref()
            .map(|current_head| HashType::BlockHash.bytes_to_string(&current_head.header().hash))
            .unwrap_or(genesis_hash)
    })
}


/// This submodule contains service functions implementation.
mod fns {
    use std::collections::{HashMap, HashSet};
    use std::convert::TryInto;
    use itertools::Itertools;
    use serde::{Deserialize, Serialize};

    use failure::{bail, format_err};
    
    use shell::shell_channel::BlockApplied;
    use shell::stats::memory::{Memory, MemoryData, MemoryStatsResult};
    use storage::{BlockHeaderWithHash, BlockStorage, BlockStorageReader, ContextRecordValue, ContextStorage};
    use storage::block_storage::BlockJsonData;
    use storage::persistent::PersistentStorage;
    use storage::skip_list::Bucket;
    use tezos_context::channel::ContextAction;
    
    use crate::ContextList;
    use crate::encoding::context::ContextConstants;
    use crate::encoding::conversions::{
        contract_id_to_address,
        chain_id_to_string
    };
    use crate::merge_slices;
    use crate::helpers::{FullBlockInfo, BlockHeaderInfo, PagedResult, RpcResponseData, EndorsingRight, RightsContextData, RightsParams, RightsConstants, EndorserSlots,
        get_prng_number, init_prng, get_level_by_block_id, get_block_hash_by_block_id};
    use crate::rpc_actor::RpcCollectedStateRef;
    use crate::ts_to_rfc3339;

    use crate::helpers::BakingRights;

    // Serialize, Deserialize,
    #[derive(Serialize, Deserialize, Debug)]
    pub struct Cycle
    {
        #[serde(skip_serializing_if = "Option::is_none")]
        last_roll: Option<HashMap<String,String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        nonces: Option<HashMap<String,String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        random_seed: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        roll_snapshot: Option<String>,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct CycleJson { 
        roll_snapshot: Option<usize>,
        random_seed: Option<String>,
    }

    /// Retrieve blocks from database.
    pub(crate) fn get_blocks(every_nth_level: Option<i32>, block_id: &str, limit: usize, persistent_storage: &PersistentStorage, state: RpcCollectedStateRef) -> Result<Vec<FullBlockInfo>, failure::Error> {
        let block_storage = BlockStorage::new(persistent_storage);
        let block_hash = get_block_hash_by_block_id(block_id, persistent_storage)?;
        let blocks = match every_nth_level {
            Some(every_nth_level) => block_storage.get_every_nth_with_json_data(every_nth_level, &block_hash, limit),
            None => block_storage.get_multiple_with_json_data(&block_hash, limit),
        }?.into_iter().map(|(header, json_data)| map_header_and_json_to_full_block_info(header, json_data, &state)).collect();
        Ok(blocks)
    }

    /// Get actions for a specific block in ascending order.
    pub(crate) fn get_block_actions(block_id: &str, persistent_storage: &PersistentStorage) -> Result<Vec<ContextAction>, failure::Error> {
        let context_storage = ContextStorage::new(persistent_storage);
        let block_hash = get_block_hash_by_block_id(block_id, persistent_storage)?;
        context_storage.get_by_block_hash(&block_hash)
            .map(|values| values.into_iter().map(|v| v.into_action()).collect())
            .map_err(|e| e.into())
    }

    /// Get actions for a specific contract in ascending order.
    pub(crate) fn get_contract_actions(contract_id: &str, from_id: Option<u64>, limit: usize, persistent_storage: &PersistentStorage) -> Result<PagedResult<Vec<ContextRecordValue>>, failure::Error> {
        let context_storage = ContextStorage::new(persistent_storage);
        let contract_address = contract_id_to_address(contract_id)?;
        let mut context_records = context_storage.get_by_contract_address(&contract_address, from_id, limit + 1)?;
        let next_id = if context_records.len() > limit { context_records.last().map(|rec| rec.id()) } else { None };
        context_records.truncate(std::cmp::min(context_records.len(), limit));
        Ok(PagedResult::new(context_records, next_id, limit))
    }

    /// Return generated baking rights.
    ///
    /// # Arguments
    /// 
    /// * `chain_id` - Url path parameter 'chain_id'.
    /// * `block_id` - Url path parameter 'block_id', it contains string "head", block level or block hash.
    /// * `level` - Url query parameter 'level'.
    /// * `delegate` - Url query parameter 'delegate'.
    /// * `cycle` - Url query parameter 'cycle'.
    /// * `max_priority` - Url query parameter 'max_priority'.
    /// * `has_all` - Url query parameter 'all'.
    /// * `list` - Context list handler.
    /// * `persistent_storage` - Persistent storage handler.
    /// * `state` - Current RPC state (head).
    /// 
    /// Prepare all data to generate baking rights and then use Tezos PRNG to generate them.
    pub(crate) fn check_and_get_baking_rights(chain_id: &str, block_id: &str, level: &Option<String>, delegate: &Option<String>, cycle: &Option<String>, max_priority: &Option<String>, has_all: bool, list: ContextList, persistent_storage: &PersistentStorage, state: RpcCollectedStateRef) -> Result<Option< RpcResponseData >, failure::Error> {

        // get block level first
        let block_level: i64 = match get_level_by_block_id(block_id, list.clone(), persistent_storage)? {
            Some(val) => val.try_into()?,
            None => bail!("Block level not found")
        };

        let constants: RightsConstants = get_and_parse_rights_constants(&chain_id, &block_id, block_level, list.clone(), persistent_storage)?;
        
        let params: RightsParams = RightsParams::parse_rights_parameters(chain_id, level, delegate, cycle, max_priority, has_all, block_level, &constants, persistent_storage, true)?;
        
        let context_data: RightsContextData = RightsContextData::prepare_context_data_for_rights(params.clone(), constants.clone(), list)?;

        get_baking_rights(&context_data, &params, &constants)
    }

    /// Use prepared data to generate baking rights
    /// 
    /// # Arguments
    /// 
    /// * `context_data` - Data from context list used in baking and endorsing rights generation filled in [RightsContextData](RightsContextData::prepare_context_data_for_rights).
    /// * `parameters` - Parameters created by [RightsParams](RightsParams::parse_rights_parameters).
    /// * `constants` - Context constants used in baking and endorsing rights [`get_and_parse_rights_constants`].
    pub(crate) fn get_baking_rights(context_data: &RightsContextData, parameters: &RightsParams, constants: &RightsConstants) -> Result<Option< RpcResponseData >, failure::Error> {
        let mut baking_rights = Vec::<BakingRights>::new();

        let blocks_per_cycle = *constants.blocks_per_cycle();
        let time_between_blocks = constants.time_between_blocks();

        let timestamp = parameters.block_timestamp();
        let block_level = parameters.block_level();

        // iterate through the whole cycle if necessery
        if let Some(cycle) = parameters.requested_cycle() {
            let first_block_level = cycle * blocks_per_cycle + 1;
            let last_block_level = first_block_level + blocks_per_cycle;
            
            for level in first_block_level..last_block_level {
                let seconds_to_add = (level - block_level).abs() * time_between_blocks[0];
                let estimated_timestamp = timestamp + seconds_to_add;

                // assign rolls goes here
                let level_baking_rights = baking_rights_assign_rolls(&parameters, &constants, &context_data, level, estimated_timestamp)?;

                baking_rights = merge_slices!(&baking_rights, &level_baking_rights);
            }
        } else {
            let level = *parameters.requested_level();
            let seconds_to_add = (level - block_level).abs() * time_between_blocks[0];
            let estimated_timestamp = timestamp + seconds_to_add;
            // assign rolls goes here
            baking_rights = baking_rights_assign_rolls(&parameters, &constants, &context_data, level, estimated_timestamp)?;
        }

        // if there is some delegate specified, retrive his priorities
        if let Some(delegate) = parameters.requested_delegate() {
            Ok(Some(RpcResponseData::BakingRights(baking_rights.into_iter().filter(|val| val.delegate().contains(delegate)).collect::<Vec<BakingRights>>())))
        } else {
            Ok(Some(RpcResponseData::BakingRights(baking_rights)))
        }
    }

    /// Use prepared data to generate baking rights
    /// 
    /// # Arguments
    /// 
    /// * `parameters` - Parameters created by [RightsParams](RightsParams::parse_rights_parameters).
    /// * `constants` - Context constants used in baking and endorsing rights [`get_and_parse_rights_constants`].
    /// * `context_data` - Data from context list used in baking and endorsing rights generation filled in [RightsContextData](RightsContextData::prepare_context_data_for_rights).
    /// * `level` - Level to feed Tezos PRNG.
    /// * `estimated_head_timestamp` - Estimated time of baking, is set to None if in past relative to block_id.
    /// 
    /// Baking priorities are are assigned to Roles, the default behavior is to include only the top priority for the delegate
    fn baking_rights_assign_rolls(parameters: &RightsParams, constants: &RightsConstants, context_data: &RightsContextData, level: i64, estimated_head_timestamp: i64) -> Result<Vec<BakingRights>, failure::Error>{
        const BAKING_USE_STRING: &[u8] = b"level baking:";
        
        // hashset is defined to keep track of the delegates with priorities allready assigned
        let mut baking_rights = Vec::<BakingRights>::new();
        let mut assigned = HashSet::new();

        let time_between_blocks = constants.time_between_blocks();

        let max_priority = *parameters.max_priority();
        let has_all = parameters.has_all();
        let block_level = *parameters.block_level();

        for priority in 0..max_priority {
            // draw the rolls for the requested parameters
            let delegate_to_assign;
            // TODO: priority can overflow in the ocaml code, do a priority % i32::max_value()
            let mut state = init_prng(&context_data, &constants, BAKING_USE_STRING, level.try_into()?, priority.try_into()?)?;

            loop {
                let (random_num, sequence) = get_prng_number(state, *context_data.last_roll())?;

                if let Some(d) = context_data.rolls().get(&random_num) {
                    delegate_to_assign = d;
                    break;
                } else {
                    state = sequence;
                }
            }
            
            // if the delegate was assgined and the the has_all flag is not set skip this priority
            if assigned.contains(&delegate_to_assign) && !has_all {
                continue;
            }

            // we omit the estimated_time field if the block on the requested level is allready baked
            if block_level < level {
                let priority_timestamp = estimated_head_timestamp + (priority as i64 * time_between_blocks[1]);
                baking_rights.push(BakingRights::new(level, delegate_to_assign.to_string(), priority.into(), Some(ts_to_rfc3339(priority_timestamp))));
            } else {
                baking_rights.push(BakingRights::new(level, delegate_to_assign.to_string(), priority.into(), None))
            }
            assigned.insert(delegate_to_assign);
        }
        Ok(baking_rights)
    }

    /// Return generated endorsing rights.
    ///
    /// # Arguments
    /// 
    /// * `chain_id` - Url path parameter 'chain_id'.
    /// * `block_id` - Url path parameter 'block_id', it contains string "head", block level or block hash.
    /// * `level` - Url query parameter 'level'.
    /// * `delegate` - Url query parameter 'delegate'.
    /// * `cycle` - Url query parameter 'cycle'.
    /// * `has_all` - Url query parameter 'all'.
    /// * `list` - Context list handler.
    /// * `persistent_storage` - Persistent storage handler.
    /// * `state` - Current RPC state (head).
    /// 
    /// Prepare all data to generate endorsing rights and then use Tezos PRNG to generate them.
    pub(crate) fn check_and_get_endorsing_rights(chain_id: &str, block_id: &str, level: &Option<String>, delegate: &Option<String>, cycle: &Option<String>, has_all: bool, list: ContextList, persistent_storage: &PersistentStorage, state: RpcCollectedStateRef) -> Result<Option< RpcResponseData >, failure::Error> {
        
        // get block level from block_id and from now get all nessesary data by block level
        let block_level: i64 = match get_level_by_block_id(block_id, list.clone(), persistent_storage)? {
            Some(val) => val.try_into()?,
            None => bail!("Block level not found")
        };

        let constants: RightsConstants = get_and_parse_rights_constants(chain_id, block_id, block_level, list.clone(), persistent_storage)?;

        let params: RightsParams = RightsParams::parse_rights_parameters(chain_id, level, delegate, cycle, &None, has_all, block_level, &constants, persistent_storage, false)?;
        
        let context_data: RightsContextData = RightsContextData::prepare_context_data_for_rights(params.clone(), constants.clone(), list)?;

        get_endorsing_rights(&context_data, &params, &constants)
    }

    /// Use prepared data to generate endosring rights
    /// 
    /// # Arguments
    /// 
    /// * `context_data` - Data from context list used in baking and endorsing rights generation filled in [RightsContextData](RightsContextData::prepare_context_data_for_rights).
    /// * `parameters` - Parameters created by [RightsParams](RightsParams::parse_rights_parameters).
    /// * `constants` - Context constants used in baking and endorsing rights [`get_and_parse_rights_constants`].
    fn get_endorsing_rights(context_data: &RightsContextData, parameters: &RightsParams, constants: &RightsConstants) -> Result<Option< RpcResponseData >, failure::Error> {
        
        // define helper and output variables
        let mut endorsing_rights = Vec::<EndorsingRight>::new();

        // when query param cycle is specified then iterate over all cycle levels, else only given level
        if let Some(cycle) = parameters.requested_cycle() {
            let blocks_per_cycle = *constants.blocks_per_cycle();
            let first_cycle_level = cycle * blocks_per_cycle + 1;
            let last_cycle_level = first_cycle_level + blocks_per_cycle;
            for level in first_cycle_level..last_cycle_level {
                // get estimated time first because is equal for all endorsers in given level
                // the base level for estimated time computation is level of previous block
                let estimated_time: Option<String> = parameters.get_estimated_time(constants, Some(level-1));

                let level_endorsing_rights = complete_endorsing_rights_for_level(context_data, parameters, constants, level, level, estimated_time)?;
                endorsing_rights = merge_slices!(&endorsing_rights, &level_endorsing_rights);
            }
        } else {
            // use level prepared during parameter parsing to compute estimated time
            let estimated_time: Option<String> = parameters.get_estimated_time(constants, None);

            endorsing_rights = complete_endorsing_rights_for_level(context_data, parameters, constants, *parameters.requested_level(), *parameters.display_level(), estimated_time)?;
        };

        Ok(Some(RpcResponseData::EndorsingRights(endorsing_rights)))
    }
    
    /// Use prepared data to generate endosring rights
    /// 
    /// # Arguments
    /// 
    /// * `context_data` - Data from context list used in baking and endorsing rights generation filled in [RightsContextData](RightsContextData::prepare_context_data_for_rights).
    /// * `parameters` - Parameters created by [RightsParams](RightsParams::parse_rights_parameters).
    /// * `constants` - Context constants used in baking and endorsing rights [`get_and_parse_rights_constants`].
    /// * `level` - Level to feed Tezos PRNG.
    /// * `display_level` - Level to be displayed in output.
    /// * `estimated_time` - Estimated time of endorsement, is set to None if in past relative to block_id.
    #[inline]
    fn complete_endorsing_rights_for_level(context_data: &RightsContextData, parameters: &RightsParams, constants: &RightsConstants, level: i64, display_level: i64, estimated_time: Option<String>) -> Result<Vec::<EndorsingRight>, failure::Error> {
        let mut endorsing_rights = Vec::<EndorsingRight>::new();

        // endorsers_slots is needed to group all slots by delegate
        let endorsers_slots = get_endorsers_slots(constants, context_data, level)?;

        // order descending by delegate public key hash address hex byte string
        for delegate in endorsers_slots.keys().sorted().rev() {
            let delegate_data = endorsers_slots.get(delegate).ok_or(format_err!("missing EndorserSlots"))?;

            // prepare delegate contract id
            let delegate_contract_id = delegate_data.contract_id().to_string();

            // filter delegates
            if let Some(d) = parameters.requested_delegate() {
                if delegate_contract_id != d.to_string() {
                    continue;
                }
            }

            endorsing_rights.push(EndorsingRight::new(
                display_level,
                delegate_contract_id,
                delegate_data.slots().clone(),
                estimated_time.clone())
            )
        }
        Ok(endorsing_rights)
    }

    /// Use tezos PRNG to collect all slots for each endorser by public key hash (for later ordering of endorsers)
    ///
    /// # Arguments
    /// 
    /// * `constants` - Context constants used in baking and endorsing rights [`get_and_parse_rights_constants`].
    /// * `context_data` - Data from context list used in baking and endorsing rights generation filled in [RightsContextData](RightsContextData::prepare_context_data_for_rights).
    /// * `level` - Level to feed Tezos PRNG.
    #[inline]
    fn get_endorsers_slots(constants: &RightsConstants, context_data: &RightsContextData, level: i64) -> Result<HashMap<String, EndorserSlots>, failure::Error> {
        // special byte string used in Tezos PRNG
        const ENDORSEMENT_USE_STRING: &[u8] = b"level endorsement:";
        // prepare helper variable
        let mut endorsers_slots: HashMap<String, EndorserSlots> = HashMap::new();

        for endorser_slot in (0 .. *constants.endorsers_per_block() as u8).rev() {
            // generate PRNG per endorsement slot and take delegates by roll number from context_rolls
            // if roll number is not found then reroll with new state till roll nuber is found in context_rolls
            let mut state = init_prng(&context_data, &constants, ENDORSEMENT_USE_STRING, level.try_into()?, endorser_slot.try_into()?)?;
            loop {
                let (random_num, sequence) = get_prng_number(state, *context_data.last_roll())?;

                if let Some(delegate) = context_data.rolls().get(&random_num) {
                    // collect all slots for each delegate
                    // convert contract id to public key hash address hex byte string (needed for later ordering)
                    let public_key_hash = hex::encode(contract_id_to_address(&delegate)?);
                    let endorsers_slots_entry = endorsers_slots.entry(public_key_hash).or_insert( EndorserSlots::new(delegate.clone(),Vec::new()) );
                    endorsers_slots_entry.push_to_slot(endorser_slot);
                    break;
                } else {
                    state = sequence;
                }
            }
        }
        Ok(endorsers_slots)
    }

    /// Get information about current head
    pub(crate) fn get_full_current_head(state: RpcCollectedStateRef) -> Result<Option<FullBlockInfo>, failure::Error> {
        let state = state.read().unwrap();
        let current_head = state.current_head().as_ref().map(|current_head| {
            let chain_id = chain_id_to_string(state.chain_id());
            FullBlockInfo::new(current_head, &chain_id)
        });

        Ok(current_head)
    }

    /// Get information about current head header
    pub(crate) fn get_current_head_header(state: RpcCollectedStateRef) -> Result<Option<BlockHeaderInfo>, failure::Error> {
        let state = state.read().unwrap();
        let current_head = state.current_head().as_ref().map(|current_head| {
            let chain_id = chain_id_to_string(state.chain_id());
            BlockHeaderInfo::new(current_head, &chain_id)
        });

        Ok(current_head)
    }

    /// Get information about block
    pub(crate) fn get_full_block(block_id: &str, persistent_storage: &PersistentStorage, state: RpcCollectedStateRef) -> Result<Option<FullBlockInfo>, failure::Error> {
        let block_storage = BlockStorage::new(persistent_storage);
        let block_hash = get_block_hash_by_block_id(block_id, persistent_storage)?;
        let block = block_storage.get_with_json_data(&block_hash)?.map(|(header, json_data)| map_header_and_json_to_full_block_info(header, json_data, &state));

        Ok(block)
    }

    /// Get information about block header
    pub(crate) fn get_block_header(block_id: &str, persistent_storage: &PersistentStorage, state: RpcCollectedStateRef) -> Result<Option<BlockHeaderInfo>, failure::Error> {
        let block_storage = BlockStorage::new(persistent_storage);
        let block_hash = get_block_hash_by_block_id(block_id, persistent_storage)?;
        let block = block_storage.get_with_json_data(&block_hash)?.map(|(header, json_data)| map_header_and_json_to_block_header_info(header, json_data, &state));

        Ok(block)
    }

    /// Get protocol context constants from context list
    /// 
    /// # Arguments
    /// 
    /// * `chain_id` - id of chain, not used
    /// * `block_id` - Url path parameter 'block_id', it contains string "head", block level or block hash.
    /// * `opt_level` - Optionaly input block level from block_id if is already known to prevent double code execution.
    /// * `list` - Context list handler.
    /// * `persistent_storage` - Persistent storage handler.
    pub(crate) fn get_context_constants(_chain_id: &str, block_id: &str, opt_level: Option<i64>, list: ContextList, persistent_storage: &PersistentStorage) -> Result<Option<ContextConstants>, failure::Error> {
        // first check if level is already known
        let level: usize = if let Some(l) = opt_level {
            l.try_into()?
        } else {
            // get level level by block_id
            if let Some(l) = get_level_by_block_id(block_id, list.clone(), persistent_storage)? {
                l
            } else {
                bail!("Level not found for block_id {}", block_id)
            }
        };

        let protocol_hash: Vec<u8>;
        let constants: Vec<u8>;
        {
            let reader = list.read().unwrap();
            if let Some(Bucket::Exists(data)) = reader.get_key(level, &"protocol".to_string())? {
                protocol_hash = data;
            } else {
                panic!(format!("Protocol not found in block: {}", block_id))
            }

            if let Some(Bucket::Exists(data)) = reader.get_key(level, &"data/v1/constants".to_string())? {
                constants = data;
            } else {
                constants = Default::default();
            }
        };

        Ok(Some(ContextConstants::transpile_context_bytes(&constants, protocol_hash)?))
    }


    pub(crate) fn get_cycle_from_context(level: &str, list: ContextList) -> Result<Option<HashMap<String, Cycle>>, failure::Error> {

        let ctxt_level: usize = level.parse().unwrap();

        let context_data = {
            let reader = list.read().expect("mutex poisoning");
            if let Ok(Some(c)) = reader.get(ctxt_level) {
                c
            } else {
                bail!("Context data not found")
            }
        };

        // get cylce list from context storage
        let cycle_lists: HashMap<String, Bucket<Vec<u8>>> = context_data.clone().into_iter()
            .filter(|(k, _)| k.contains("cycle"))
            .filter(|(_, v)| match v { Bucket::Exists(_) => true, _ => false })
            .collect();
        
        // transform cycle list     
        let mut cycles: HashMap<String,Cycle> = HashMap::new();

        // process every key value pair
        for (key, bucket) in cycle_lists.iter() {
            
            // create vector from path
            let path:Vec<&str> = key.split('/').collect();
            
            // convert value from bytes to hex 
            let value = match bucket { 
                Bucket::Exists(value) => hex::encode(value).to_string(),
                _ => "".to_string()
            };

            // create new cycle
            // TODO: !!! check path and move to match 
            cycles.entry(path[2].to_string()).or_insert(Cycle {
                last_roll: None,
                nonces: None,
                random_seed: None,
                roll_snapshot: None,
            });

            // process cycle key value pairs     
            match path.as_slice() {
                ["data", "cycle", cycle, "random_seed"] => {
                    // println!("cycle: {:?} random_seed: {:?}", cycle, value );
                    cycles.entry(cycle.to_string()).and_modify(|cycle| {
                        cycle.random_seed = Some(value);
                    });
                },
                ["data", "cycle", cycle, "roll_snapshot"] => {
                    // println!("cycle: {:?} roll_snapshot: {:?}", cycle, value);
                    cycles.entry(cycle.to_string()).and_modify(|cycle| {
                        cycle.roll_snapshot = Some(value);
                    });
                },
                ["data", "cycle", cycle, "nonces", nonces] => {
                    // println!("cycle: {:?} nonces: {:?}/{:?}", cycle, nonces, value)
                    cycles.entry(cycle.to_string()).and_modify(|cycle| {
                        match cycle.nonces.as_mut() {
                            Some(entry) => entry.insert(nonces.to_string(),value),
                            None => { 
                                cycle.nonces = Some(HashMap::new());
                                cycle.nonces.as_mut().unwrap().insert(nonces.to_string(),value)
                            }
                        };
                    });
                },
                ["data", "cycle", cycle, "last_roll", last_roll] => {
                    // println!("cycle: {:?} last_roll: {:?}/{:?}", cycle, last_roll, value)
                    cycles.entry(cycle.to_string()).and_modify(|cycle| {
                        match cycle.last_roll.as_mut() {
                            Some(entry) => entry.insert(last_roll.to_string(),value),
                            None => { 
                                cycle.last_roll = Some(HashMap::new());
                                cycle.last_roll.as_mut().unwrap().insert(last_roll.to_string(),value)
                            }
                        };                    
                    });
                },
                _ => ()
            };
        }
       
        Ok(Some(cycles))
    } 

    pub(crate) fn get_cycle_from_context_as_json(level: &str, cycle_id: &str, list: ContextList) -> Result<Option<CycleJson>, failure::Error> {

        let ctxt_level: usize = level.parse().unwrap();

        let context_data = {
            let reader = list.read().expect("mutex poisoning");
            if let Ok(Some(c)) = reader.get(ctxt_level) {
                c
            } else {
                bail!("Context data not found")
            }
        };

        let patch_cycle = format!("cycle/{}", &cycle_id );
        // get cylce list from context storage
        let cycle_lists: HashMap<String, Bucket<Vec<u8>>> = context_data.clone().into_iter()
            .filter(|(k, _)| k.contains(&patch_cycle))
            .filter(|(_, v)| match v { Bucket::Exists(_) => true, _ => false })
            .collect();

       
        let mut cycle = CycleJson {
            roll_snapshot: None,
            random_seed: None,
        };

        // process every key value pair
        for (key, bucket) in cycle_lists.iter() {
            
            // create vector from path
            let path:Vec<&str> = key.split('/').collect();
            
            // convert value from bytes to hex 
            let value = match bucket { 
                Bucket::Exists(value) => hex::encode(value).to_string(),
                _ => "".to_string()
            };
            
            // process cycle key value pairs     
            match path.as_slice() {
                ["data", "cycle", _, "random_seed"] => {
                    cycle.random_seed = Some(value);
                },
                ["data", "cycle", _, "roll_snapshot"] => {
                    cycle.roll_snapshot = Some(value.parse().unwrap());
                },
                _ => ()
            };
        }
       
        Ok(Some(cycle))
    }

    pub(crate) fn get_rolls_owner_current_from_context(level: &str, list: ContextList) -> Result<Option<HashMap<String,HashMap<String,HashMap<String,String>>>>, failure::Error> {

        let ctxt_level: usize = level.parse().unwrap();
        // println!("level: {:?}", ctxt_level);

        let context_data = {
            let reader = list.read().expect("mutex poisoning");
            if let Ok(Some(c)) = reader.get(ctxt_level) {
                c
            } else {
                bail!("Context data not found")
            }
        };

        // get rolls list from context storage
        let rolls_lists: HashMap<String, Bucket<Vec<u8>>> = context_data.clone().into_iter()
            .filter(|(k, _)| k.contains("rolls/owner/current"))
            .filter(|(_, v)| match v { Bucket::Exists(_) => true, _ => false })
            .collect();

        // create rolls list     
        let mut rolls: HashMap<String,HashMap<String,HashMap<String,String>>> = HashMap::new();

        // process every key value pair
        for (key, bucket) in rolls_lists.iter() {
            
            // create vector from path
            let path:Vec<&str> = key.split('/').collect();

            // convert value from bytes to hex 
            let value = match bucket { 
                Bucket::Exists(value) => hex::encode(value).to_string(),
                _ => "".to_string()
            };

            // process roll key value pairs     
            match path.as_slice() {
                ["data", "rolls","owner","current", path1, path2, path3 ] => {
                    // println!("rolls: {:?}/{:?}/{:?} value: {:?}", path1, path2, path3, value );

                    rolls.entry(path1.to_string())
                        .and_modify(|roll| {
                            roll.entry(path2.to_string())
                                .and_modify(|index2| { 
                                    index2.insert(path3.to_string(),value.to_string());
                                })
                                .or_insert({
                                    let mut index3 = HashMap::new();
                                    index3.insert(path3.to_string(),value.to_string());
                                    index3
                                });
                        })
                        .or_insert({
                            let mut index2 = HashMap::new();
                            index2.entry(path2.to_string())
                                .or_insert({
                                    let mut index3 = HashMap::new();
                                    index3.insert(path3.to_string(),value.to_string());
                                    index3  
                                });
                            index2
                        });    
            
                },
                _ => ()
            }

        }
        
    
        Ok(Some(rolls))

    }

    pub(crate) fn get_stats_memory() -> MemoryStatsResult<MemoryData> {
        let memory = Memory::new();
        memory.get_memory_stats()
    }

    pub(crate) fn get_context(level: &str, list: ContextList) -> Result<Option<HashMap<String, Bucket<Vec<u8>>>>, failure::Error> {
        let level = level.parse()?;
        {
            let storage = list.read().expect("poisoned storage lock");
            storage.get(level).map_err(|e| e.into())
        }
    }

    /// Get all context constants which are used in endorsing and baking rights generation
    /// 
    /// # Arguments
    /// 
    /// * `chain_id` - Url path parameter 'chain_id'.
    /// * `block_id` - Url path parameter 'block_id', it contains string "head", block level or block hash.
    /// * `block_level` - Provide level of block_id that is already known to prevent double code execution.
    /// * `persistent_storage` - Persistent storage handler.
    #[inline]
    fn get_and_parse_rights_constants(chain_id: &str, block_id: &str, block_level: i64, list: ContextList, persistent_storage: &PersistentStorage) -> Result<RightsConstants, failure::Error> {
        let constants = match get_context_constants(chain_id, block_id, Some(block_level), list.clone(), persistent_storage)? {
            Some(v) => v,
            None => bail!("Cannot get protocol constants")
        };

        Ok(RightsConstants::new(
            *constants.blocks_per_cycle(),
            *constants.preserved_cycles(),
            *constants.nonce_length(),
            constants.time_between_blocks().to_vec().into_iter().map(|x| x.parse().unwrap()).collect(),
            *constants.blocks_per_roll_snapshot(),
            *constants.endorsers_per_block(),
        ))
    }

    #[inline]
    fn map_header_and_json_to_full_block_info(header: BlockHeaderWithHash, json_data: BlockJsonData, state: &RpcCollectedStateRef) -> FullBlockInfo {
        let state = state.read().unwrap();
        let chain_id = chain_id_to_string(state.chain_id());
        FullBlockInfo::new(&BlockApplied::new(header, json_data), &chain_id)
    }

    #[inline]
    fn map_header_and_json_to_block_header_info(header: BlockHeaderWithHash, json_data: BlockJsonData, state: &RpcCollectedStateRef) -> BlockHeaderInfo {
        let state = state.read().unwrap();
        let chain_id = chain_id_to_string(state.chain_id());
        BlockHeaderInfo::new(&BlockApplied::new(header, json_data), &chain_id)
    }

}

