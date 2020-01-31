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
    match fns::check_baking_rights(chain_id, block_id, level, delegate, cycle, max_priority, has_all, list, persistent_storage, state) {
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
    //match fns::check_baking_rights(chain_id, block_id, level, delegate, cycle, max_priority, has_all, list, persistent_storage, state) {
    match fns::check_endorsing_rights(chain_id, block_id, level, delegate, cycle, has_all, list, persistent_storage, state) {
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
    routes.insert("/chains/:chain_id/blocks/:block_id/context/raw/json/cycle", Route::ContextJsonCycle);
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
            result_to_json_response(fns::get_context_constants(chain_id, block_id, context_storage, &persistent_storage), &log)
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
            result_to_json_response(fns::get_cycle_from_context_as_json(block_id, context_storage), &log)
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

            //warn!(log, "Args = Block Id: {:?} max_priority: {:?} level: {:?} delegate: {:?} cycle {:?}", &block_id, &max_priority, level.clone().unwrap_or("".to_string()), delegate.clone().unwrap_or("".to_string()), &cycle);

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

    use crypto::hash::{BlockHash, ChainId, HashType};
    use crypto::blake2b;
    
    use shell::shell_channel::BlockApplied;
    use shell::stats::memory::{Memory, MemoryData, MemoryStatsResult};
    use storage::{BlockHeaderWithHash, BlockMetaStorage, BlockStorage, BlockStorageReader, ContextRecordValue, ContextStorage};
    use storage::block_storage::BlockJsonData;
    use storage::context_storage::ContractAddress;
    use storage::persistent::PersistentStorage;
    use storage::skip_list::Bucket;
    use storage::num_from_slice;
    use tezos_context::channel::ContextAction;
    
    use crate::ContextList;
    use crate::encoding::context::ContextConstants;
    use crate::merge_slices;
    use crate::helpers::{FullBlockInfo, BlockHeaderInfo, PagedResult, RpcResponseData, EndorsingRight, CycleData, cycle_from_level, get_pseudo_random_number, init_prng, RightsParams, RightsConstants, EndorserSlots};
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
    
    type ContextMap = HashMap<String, Bucket<Vec<u8>>>;

    /// Retrieve blocks from database.
    pub(crate) fn get_blocks(every_nth_level: Option<i32>, block_id: &str, limit: usize, persistent_storage: &PersistentStorage, state: RpcCollectedStateRef) -> Result<Vec<FullBlockInfo>, failure::Error> {
        let block_storage = BlockStorage::new(persistent_storage);
        let block_hash = block_id_to_block_hash(block_id, persistent_storage)?;
        let blocks = match every_nth_level {
            Some(every_nth_level) => block_storage.get_every_nth_with_json_data(every_nth_level, &block_hash, limit),
            None => block_storage.get_multiple_with_json_data(&block_hash, limit),
        }?.into_iter().map(|(header, json_data)| map_header_and_json_to_full_block_info(header, json_data, &state)).collect();
        Ok(blocks)
    }

    /// Get actions for a specific block in ascending order.
    pub(crate) fn get_block_actions(block_id: &str, persistent_storage: &PersistentStorage) -> Result<Vec<ContextAction>, failure::Error> {
        let context_storage = ContextStorage::new(persistent_storage);
        let block_hash = block_id_to_block_hash(block_id, persistent_storage)?;
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


    pub(crate) fn check_baking_rights(chain_id: &str, block_id: &str, level: &Option<String>, delegate: &Option<String>, cycle: &Option<String>, max_priority: &Option<String>, has_all: bool, list: ContextList, persistent_storage: &PersistentStorage, state: RpcCollectedStateRef) -> Result<Option< RpcResponseData >, failure::Error> {
        //const NO_CYCLE_FLAG: i32 = -1;

        // get the constants first!
        let constants = get_and_parse_baking_constants(&chain_id, &block_id, list.clone(), persistent_storage)?;

        let params: RightsParams = parse_rights_url_params_and_arguments(chain_id, block_id, level, delegate, cycle, max_priority, has_all, &constants, list.clone(), persistent_storage, state, true)?;
        
        let cycle_data: CycleData = get_context_data_for_rights(params.clone(), constants.clone(), list)?;

        get_baking_rights(&cycle_data, &params, &constants)
    }

    pub(crate) fn get_baking_rights(cycle_data: &CycleData, parameters: &RightsParams, constants: &RightsConstants) -> Result<Option< RpcResponseData >, failure::Error> {
        let mut baking_rights = Vec::<BakingRights>::new();

        // TODO fix casting!
        // let preserved_cycles = *constants.preserved_cycles() as i32;
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
                let level_baking_rights = baking_rights_assign_rolls(&parameters, &constants, &cycle_data, level, estimated_timestamp)?;

                baking_rights = merge_slices!(&baking_rights, &level_baking_rights);
            }
        } else {
            let level = *parameters.requested_level();
            let seconds_to_add = (level - block_level).abs() * time_between_blocks[0];
            let estimated_timestamp = timestamp + seconds_to_add;
            // println!("No cycle requested, getting rights for level {}", level);
            // assign rolls goes here
            baking_rights = baking_rights_assign_rolls(&parameters, &constants, &cycle_data, level, estimated_timestamp)?;
        }

        // if there is some delegate specified, retrive his priorities
        if let Some(delegate) = parameters.requested_delegate() {
            Ok(Some(RpcResponseData::BakingRights(baking_rights.into_iter().filter(|val| val.delegate().contains(delegate)).collect::<Vec<BakingRights>>())))
        } else {
            Ok(Some(RpcResponseData::BakingRights(baking_rights)))
        }
    }

    pub(crate) fn baking_rights_assign_rolls(parameters: &RightsParams, constants: &RightsConstants, cycle_data: &CycleData, level: i64, estimated_head_timestamp: i64) -> Result<Vec<BakingRights>, failure::Error>{
        const BAKING_USE_STRING: &[u8] = b"level baking:";
        
        // as we assign the roles, the default behavior is to include only the top priority for the delegate
        // we define a hashset to keep track of the delegates with priorities allready assigned
        let mut baking_rights = Vec::<BakingRights>::new();
        let mut assigned = HashSet::new();

        let time_between_blocks = constants.time_between_blocks();

        let max_priority = *parameters.max_priority();
        let has_all = parameters.has_all();
        let block_level = *parameters.block_level();

        for priority in 0..max_priority {
            // draw the rolls for the requested parameters
            let delegate_to_assign;
            // TODO: priority can overflow in the ocaml code, do a    priority % i32::max_value()
            let mut state = init_prng(&cycle_data, &constants, BAKING_USE_STRING, level.try_into()?, priority.try_into()?)?;

            loop {
                let (random_num, sequence) = get_pseudo_random_number(state, *cycle_data.last_roll())?;

                if let Some(d) = cycle_data.rolls().get(&random_num) {
                    delegate_to_assign = d;
                    // println!("[baking-rights] rolled {} for prio {} - assigning: {}", &random_num, &priority, &delegate_to_assign);
                    break;
                } else {
                    // println!("[baking-rights] no delegate assigned to roll {} | ROLLED ON PRIO: {}", random_num, priority);
                    state = sequence;
                }
            }
            
            // if the delegate was assgined and the the has_all flag is not set skip this priority
            if assigned.contains(&delegate_to_assign) && !has_all {
                continue;
            }

            // we omit the estimated_time field if the block on the requested level is allready baked
            if block_level < level {
                // let priority_timestamp = Some(estimated_timestamp.unwrap().checked_add_signed(Duration::seconds(priority as i64 * time_between_blocks[1])).unwrap());
                let priority_timestamp = estimated_head_timestamp + (priority as i64 * time_between_blocks[1]);
                baking_rights.push(BakingRights::new(level, delegate_to_assign.to_string(), priority.into(), Some(ts_to_rfc3339(priority_timestamp))));
            } else {
                baking_rights.push(BakingRights::new(level, delegate_to_assign.to_string(), priority.into(), None))
            }
            assigned.insert(delegate_to_assign);
        }
        Ok(baking_rights)
    }

    pub(crate) fn parse_rights_url_params_and_arguments(
        param_chain_id: &str,
        param_block_id: &str,
        param_level: &Option<String>,
        param_delegate: &Option<String>,
        param_cycle: &Option<String>,
        param_max_priority: &Option<String>,
        param_has_all: bool,
        rights_constants: &RightsConstants,
        context_list: ContextList,
        persistent_storage: &PersistentStorage,
        state: RpcCollectedStateRef,
        is_baking_rights: bool
    ) -> Result<RightsParams, failure::Error> {
        // do the parsing here and return a struct

        let block_level_usize = match get_level_by_block_id(param_block_id, context_list.clone(), persistent_storage)? {
            Some(val) => val,
            None => bail!("Block level not found")
        };

        // trying to maintain the consistent i64 in the baking rights
        let block_level: i64 = block_level_usize.try_into()?;


        let preserved_cycles = *rights_constants.preserved_cycles();
        let blocks_per_cycle = *rights_constants.blocks_per_cycle();
        // maybe refactor the whole blok_level as usize
        let current_cycle = cycle_from_level(block_level, blocks_per_cycle);

        // display_level is here because of corner case where all levels < 1 are computed as level 1 but oputputed as they are
        // and also when block_level is used for enndorsing rights
        // this level is used in final endorsing rights output
        let mut display_level: i64 = block_level;
        // level for timestamp computation, endorsing rights only
        let mut timestamp_level:i64 = block_level;
        // Check the param_level, if there is a level specified validate it, if no set it to the next level to be baked
        let requested_level: i64 = match param_level {
            Some(level) => {
                let level = level.parse()?;
                // check the bounds for the requested level (if it is in the previous/next preserved cycles)
                validate_cycle(cycle_from_level(level, blocks_per_cycle), current_cycle, preserved_cycles)?;
                display_level = level;
                timestamp_level = level-1;
                get_valid_level(level)
            },
            None => if is_baking_rights {
                    block_level + 1
                } else {
                    block_level
                }
        };


        // if we have a requested cycle validate the cycle bounds, return error message if fails
        let requested_cycle = match param_cycle {
            Some(val) => Some(validate_cycle(val.parse()?, current_cycle, preserved_cycles)?),
            None => None
        };

        // set max_priority to param value or default
        let max_priority = match param_max_priority {
            Some(val) => val.parse()?,
            None => 64
        };

        let block_timestamp = get_block_header_timestamp(param_block_id, persistent_storage, state)?;

        Ok(RightsParams::new(
            param_chain_id.to_string(),
            block_level,
            block_timestamp,
            param_delegate.clone(),
            requested_cycle,
            requested_level,
            display_level, // endorsing rights only
            timestamp_level, // endorsing rights only
            max_priority,
            param_has_all
        ))
    }

    #[inline]
    fn get_valid_level(level: i64) -> i64 {
        // for all reuqested negative levels compute with level 1
        if level < 1 {
            1
        } else {
            level
        }
    }

    pub(crate) fn validate_cycle(requested_cycle: i64, current_cycle: i64, preserved_cycles: i64) -> Result<i64, failure::Error> {
        if (requested_cycle - current_cycle).abs() <= preserved_cycles {
            Ok(requested_cycle)
        } else {
            bail!("Requested cycle out of bounds")
        }
    }

    pub(crate) fn get_and_parse_baking_constants(chain_id: &str, block_id: &str, context_list: ContextList, persistent_storage: &PersistentStorage) -> Result<RightsConstants, failure::Error> {
        let constants = match get_context_constants(chain_id, block_id, context_list.clone(), persistent_storage)? {
            Some(v) => v,
            None => bail!("Cannot get protocol constants")
        };

        Ok(RightsConstants::new(
            *constants.blocks_per_cycle(),
            *constants.preserved_cycles(),
            *constants.nonce_length(),
            constants.time_between_blocks().to_vec(),
            *constants.blocks_per_roll_snapshot(),
            *constants.endorsers_per_block(),
        ))
    }

    pub(crate) fn check_endorsing_rights(chain_id: &str, block_id: &str, level: &Option<String>, delegate: &Option<String>, cycle: &Option<String>, has_all: bool, list: ContextList, persistent_storage: &PersistentStorage, state: RpcCollectedStateRef) -> Result<Option< RpcResponseData >, failure::Error> {
        
        // get the constants first!
        let constants = get_and_parse_baking_constants(&chain_id, &block_id, list.clone(), persistent_storage)?;

        let params: RightsParams = parse_rights_url_params_and_arguments(chain_id, block_id, level, delegate, cycle, &None, has_all, &constants, list.clone(), persistent_storage, state, false)?;
        
        let cycle_data: CycleData = get_context_data_for_rights(params.clone(), constants.clone(), list)?;

        get_endorsing_rights(&cycle_data, &params, &constants)
    }

    #[inline]
    fn get_endorsing_rights(cycle_data: &CycleData, parameters: &RightsParams, constants: &RightsConstants) -> Result<Option< RpcResponseData >, failure::Error> {
        
         // prepare filter by delegate
        let mut check_delegates = false;
        let delegate_filter: String = if let Some(d) = parameters.requested_delegate() {
            check_delegates = true;
            d.to_string()
        } else {
            "".to_string() //dummy value, never used
        };

        // define constants
        let time_between_blocks = constants.time_between_blocks();
        let blocks_per_cycle = *constants.blocks_per_cycle();

        // define parameters
        let requested_level = *parameters.requested_level();
        let display_level = *parameters.display_level();
        let block_level = *parameters.block_level();
        let timestamp_level = *parameters.timestamp_level();
        let block_timestamp = *parameters.block_timestamp();

        // define helper and output variables
        let mut endorsing_rights = Vec::<EndorsingRight>::new();
        let mut is_cycle = false;

        // when query param cycle is specified then iterate over all cycle levels, else only given level
        let level_iterator = if let Some(cycle) = parameters.requested_cycle() {
            is_cycle = true;
            let first_block_level = cycle * blocks_per_cycle + 1;
            let last_block_level = first_block_level + blocks_per_cycle;
            first_block_level..last_block_level
        } else {
            requested_level..requested_level+1
        };

        for level in level_iterator {
            //check if estimated time is computed and convert from raw epoch time to rfc3339 format
            let timestamp_level = if is_cycle {
                level-1
            } else {
                timestamp_level
            };

            let estimated_time: Option<String> = if block_level <= timestamp_level {
                let est_timestamp = ((timestamp_level - block_level).abs() as i64 * time_between_blocks[0]) + block_timestamp;
                Some(ts_to_rfc3339(est_timestamp))
            } else {
                None
            };

            // endorsers_slots is needed to group all slots by delegate
            let endorsers_slots = get_endorsers_slots(&constants, &cycle_data, level)?;

            // define display level for EndorsingRight structure
            let display_level = if is_cycle {
                level
            } else {
                display_level
            };

            // order descending by delegate public key hash address hex byte string
            for delegate in endorsers_slots.keys().sorted().rev() {
                let delegate_contract_id = endorsers_slots.get(delegate).unwrap().contract_id().to_string();
                // filter delegates
                if check_delegates && delegate_contract_id != delegate_filter {
                    continue
                }
                endorsing_rights.push(EndorsingRight::new(
                    display_level, 
                    delegate_contract_id, 
                    endorsers_slots.get(delegate).unwrap().slots().clone(), 
                    estimated_time.clone())
                )
            }
        }

        Ok(Some(RpcResponseData::EndorsingRights(endorsing_rights)))
    }

    fn get_endorsers_slots(constants: &RightsConstants, cycle_data: &CycleData, level: i64) -> Result<HashMap<String, EndorserSlots>, failure::Error>{
        const ENDORSEMENT_USE_STRING: &[u8] = b"level endorsement:";
        
        // the key is delegate address and value is vector of asigned slots
        let mut endorsers_slots: HashMap<String, EndorserSlots> = HashMap::new();

        for endorser_slot in (0 .. *constants.endorsers_per_block() as u8).rev() {
            // generate PRNG per endorsement slot and take delegates by roll number from context_rolls
            // if roll number is not found then reroll with new state till roll nuber is found in context_rolls
            let mut state = init_prng(&cycle_data, &constants, ENDORSEMENT_USE_STRING, level.try_into()?, endorser_slot.try_into()?)?;
            loop {
                let (random_num, sequence) = get_pseudo_random_number(state, *cycle_data.last_roll())?;

                if let Some(delegate) = cycle_data.rolls().get(&random_num) {
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
        let block_hash = block_id_to_block_hash(block_id, persistent_storage)?;
        let block = block_storage.get_with_json_data(&block_hash)?.map(|(header, json_data)| map_header_and_json_to_full_block_info(header, json_data, &state));

        Ok(block)
    }

    /// Get information about block header
    pub(crate) fn get_block_header(block_id: &str, persistent_storage: &PersistentStorage, state: RpcCollectedStateRef) -> Result<Option<BlockHeaderInfo>, failure::Error> {
        let block_storage = BlockStorage::new(persistent_storage);
        let block_hash = block_id_to_block_hash(block_id, persistent_storage)?;
        let block = block_storage.get_with_json_data(&block_hash)?.map(|(header, json_data)| map_header_and_json_to_block_header_info(header, json_data, &state));

        Ok(block)
    }

    pub(crate) fn get_context_constants(_chain_id: &str, block_id: &str, list: ContextList, persistent_storage: &PersistentStorage) -> Result<Option<ContextConstants>, failure::Error> {
        let level: usize = if let Some(l) = get_level_by_block_id(block_id, list.clone(), persistent_storage)? {
            l
        } else {
            bail!("Level not found for block_id {}", block_id)
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
                _ => bail!("Unknown key {} value {} cycle pair", key, value)
            };
        }
       
        Ok(Some(cycles))
    }

    pub(crate) fn get_cycle_from_context_as_json(level: &str, list: ContextList) -> Result<Option<Vec<usize>>, failure::Error> {

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
        let mut cycles: Vec<usize> = vec![];

        // process every key value pair
        for (key, _) in cycle_lists.iter() {
            
            // create vector from path
            let path:Vec<&str> = key.split('/').collect();
            
            
            match path.as_slice() {
                ["data", "cycle", cycle, _] |
                ["data", "cycle", cycle, _, _ ] => {
    
                    let cycle_number: usize = cycle.parse().unwrap();
                    match cycles.iter().position(|&r| r == cycle_number) {
                        Some(_) => &cycles,
                        None => {
                            cycles.push(cycle_number);
                            // TODO: change sort order after initail bootstrap
                            cycles.sort(); 
                            &cycles
                        }
                    };
    
                },
                _ => (),
             }

        }
       
        Ok(Some(cycles))
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
                _ => bail!("Unknown key {} value {} rolls pair", key, value)
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

    #[inline]
    fn get_level_by_block_id(block_id: &str, list: ContextList, persistent_storage: &PersistentStorage) -> Result<Option<usize>, failure::Error> {
        let level = if block_id == "head" {
            let reader = list.read().expect("mutex poisoning");
            Some(reader.len() - 1)
        } else {
            // check if the block_id is defined as a number
            match block_id.parse() {
                Ok(val) => Some(val),
                Err(_e) => {
                    let block_hash = block_id_to_block_hash(block_id, persistent_storage)?;
                    let block_meta_storage: BlockMetaStorage = BlockMetaStorage::new(persistent_storage);
                    if let Some(block_meta) = block_meta_storage.get(&block_hash)? {
                        Some(block_meta.level() as usize)
                    } else {
                        None
                    }
                }
            }
        };
        Ok(level)
    }

    #[inline]
    fn get_block_header_timestamp(block_id: &str, persistent_storage: &PersistentStorage, state: RpcCollectedStateRef) -> Result<i64, failure::Error> {
        let timestamp: i64 = if block_id == "head" {
            let state_read = state.read().unwrap();
            match state_read.current_head().as_ref() {
                Some(current_head) => {
                    current_head.header().header.timestamp()
                }
                None => bail!("head not initialized")
            }
        } else {
            let block_storage = BlockStorage::new(persistent_storage);
            match block_id.parse() {
                Ok(val) => {
                    match block_storage.get_by_block_level(val)? {
                        Some(current_head) => current_head.header.timestamp(),
                        None => bail!("block not found in db {}", block_id)
                    }
                }
                Err(_e) => {
                    let block_hash = block_id_to_block_hash(block_id, persistent_storage)?;
                    match block_storage.get(&block_hash)? {
                        Some(current_head) => current_head.header.timestamp(),
                        None => bail!("block not found in db {}", block_id)
                    }
                }
            }
        };
        Ok(timestamp)
    }

    // TODO: for the fixed ctxt DB
    pub(crate) fn get_context_as_hashmap(level: usize, list: ContextList) -> Result<ContextMap, failure::Error> {
        // get the whole context
        let context = {
            let reader = list.read().unwrap();
            if let Ok(Some(ctx)) = reader.get(level) {
                ctx
            } else {
                bail!("Context not found")
            }
        };
        Ok(context)
    }

    #[inline]
    fn get_context_rolls(context: HashMap<String, Bucket<Vec<u8>>>) -> Result<Option<HashMap<i32, String>>, failure::Error> {
        let data: HashMap<String, Bucket<Vec<u8>>> = context.into_iter()
            // .filter(|(k, _)| k.contains(&format!("data/rolls/owner/snapshot/{}/{}", cycle, snapshot)))
            .filter(|(k, _)| k.contains(&"data/rolls/owner/current"))  // REMOVE THIS
            .collect();
            
        let mut roll_owners: HashMap<i32, String> = HashMap::new();

        // iterate through all the owners,the roll_num is the last component of the key, decode the value (it is a public key) to get the pkh address (tz1...)
        for (key, value) in data.into_iter() {
            let key_split = key.split('/').collect::<Vec<_>>();
            let roll_num = key_split[key_split.len() - 1];

            // the values are public keys
            if let Bucket::Exists(pk) = value {
                let delegate = public_key_to_contract_id(pk)?;
                //let delegate = hex::encode(pk);
                roll_owners.insert(roll_num.parse().unwrap(), delegate);
            } else {
                continue;  // If the val is Deleted, we just skip it and go to the next iteration
            }
        }
        Ok(Some(roll_owners))
    }

    // TODO-FIRST-THING: pass the 2 hashmaps instead of the ContextList
    // get cycle data roll_snapshot, random_seed, last_roll and rolls from context list
    // for now there are computed levels to get these data for each key, when ctx list will be fixed these keys should be available in requested_level
    #[inline]
    fn get_context_data_for_rights(parameters: RightsParams, constants: RightsConstants, list: ContextList) -> Result<CycleData, failure::Error> {
        // prepare constants
        let blocks_per_cycle = *constants.blocks_per_cycle();
        let preserved_cycles = *constants.preserved_cycles();
        let blocks_per_roll_snapshot = *constants.blocks_per_roll_snapshot();
        
        let block_level = *parameters.block_level();
        let requested_level = *parameters.requested_level();

        // let cycle = cycle_from_level(requested_level, blocks_per_cycle);
        // TODO: refactor this
        let requested_cycle;
        if let Some(cycle) = *parameters.requested_cycle() {
            requested_cycle = cycle;
        } else {
            requested_cycle = cycle_from_level(requested_level, blocks_per_cycle);
        };

        let current_context = get_context_as_hashmap(block_level as usize, list.clone())?;
        
        // get index of roll snapshot
        let roll_snapshot: i16 = {
            let snapshot_key = format!("data/cycle/{}/roll_snapshot", requested_cycle);
            // println!("get snapshot_key: level:{} ctx key:{}", block_level, snapshot_key);
            if let Some(Bucket::Exists(data)) = current_context.get(&snapshot_key) {
                num_from_slice!(data, 0, i16)
            } else { // key not found
                return Err(format_err!("roll_snapshot"))
            }
        };

        let random_seed_key = format!("data/cycle/{}/random_seed", requested_cycle);
        // println!("get random_seed: level:{} ctx key:{}", block_level, random_seed_key);
        let random_seed = {
            if let Some(Bucket::Exists(data)) = current_context.get(&random_seed_key) {
                data
            } else { // key not found
                return Err(format_err!("random_seed"))
            }
        };

        let snapshot_level;
        if requested_cycle < preserved_cycles+2 {
            snapshot_level = block_level;
        } else {
            let cycle_of_rolls = requested_cycle - preserved_cycles - 2;
            // to calculate order of snapshot add 1 to snapshot index (roll_snapshot)
            snapshot_level = (cycle_of_rolls * blocks_per_cycle) + (((roll_snapshot + 1) as i64) * blocks_per_roll_snapshot) - 1;
        };

        // Snapshots of last_roll are listed from 0 same as roll_snapshot.
        let last_roll_key = format!("data/cycle/{}/last_roll/{}", requested_cycle, roll_snapshot);
        // println!("get last_roll: level:{} ctx key:{}", block_level, last_roll_key);
        let last_roll = {
            if let Some(Bucket::Exists(data)) = current_context.get(&last_roll_key) {
                num_from_slice!(data, 0, i32)
            } else { // key not found
                return Err(format_err!("last_roll"))
            }
        };
        
        let roll_context = get_context_as_hashmap(snapshot_level as usize, list.clone())?;

        // get list of rolls from context list
        // println!("get_context_rolls: level:{}", snapshot_level);
        let context_rolls = if let Some(rolls) = get_context_rolls(roll_context)? {
            rolls
        } else {
            return Err(format_err!("rolls"))
        };

        Ok(CycleData::new(
            random_seed.to_vec(),
            last_roll,
            context_rolls
        ))
    }

    /// Get block has bytes from block hash or block level
    fn block_id_to_block_hash(block_id: &str, persistent_storage: &PersistentStorage) -> Result<BlockHash, failure::Error> {
        
        let block_storage = BlockStorage::new(persistent_storage);
        let block_hash = match block_id.parse() {
            Ok(value) =>  match block_storage.get_by_block_level(value)? {
                Some(current_head) => current_head.hash,
                None => vec![]
            },
            Err(_e) => HashType::BlockHash.string_to_bytes(block_id)?   
        };

        Ok(block_hash)
    }

    #[inline]
    fn contract_id_to_address(contract_id: &str) -> Result<ContractAddress, failure::Error> {
        let contract_address = {
            if contract_id.len() == 44 {
                hex::decode(contract_id)?
            } else if contract_id.len() > 3 {
                let mut contract_address = Vec::with_capacity(22);
                match &contract_id[0..3] {
                    "tz1" => {
                        contract_address.extend(&[0, 0]);
                        contract_address.extend(&HashType::ContractTz1Hash.string_to_bytes(contract_id)?);
                    }
                    "tz2" => {
                        contract_address.extend(&[0, 1]);
                        contract_address.extend(&HashType::ContractTz2Hash.string_to_bytes(contract_id)?);
                    }
                    "tz3" => {
                        contract_address.extend(&[0, 2]);
                        contract_address.extend(&HashType::ContractTz3Hash.string_to_bytes(contract_id)?);
                    }
                    "KT1" => {
                        contract_address.push(1);
                        contract_address.extend(&HashType::ContractKt1Hash.string_to_bytes(contract_id)?);
                        contract_address.push(0);
                    }
                    _ => bail!("Invalid contract id")
                }
                contract_address
            } else {
                bail!("Invalid contract id");
            }
        };

        Ok(contract_address)
    }

    // returns the contract_id from the public_key 
    #[inline]
    fn public_key_to_contract_id(pk: Vec<u8>) -> Result<String, failure::Error> {
        // 1 byte tag and - 32 bytes for ed25519 (tz1)
        //                - 33 bytes for secp256k1 (tz2) and p256 (tz3)
        if pk.len() == 33 || pk.len() == 34 {
            let tag = pk[0];
            let hash = blake2b::digest_160(&pk[1..]);

            let contract_id = match tag {
                0 => {
                    HashType::ContractTz1Hash.bytes_to_string(&hash)
                }
                1 => {
                    HashType::ContractTz2Hash.bytes_to_string(&hash)
                }
                2 => {
                    HashType::ContractTz3Hash.bytes_to_string(&hash)
                }
                _ => bail!("Invalid public key")
            };
            Ok(contract_id)
        } else {
            bail!("Invalid public key")
        }
    }

    #[inline]
    fn chain_id_to_string(chain_id: &ChainId) -> String {
        HashType::ChainId.bytes_to_string(chain_id)
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

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_contract_id_to_address() -> Result<(), failure::Error> {
            let result = contract_id_to_address("0000cf49f66b9ea137e11818f2a78b4b6fc9895b4e50")?;
            assert_eq!(result, hex::decode("0000cf49f66b9ea137e11818f2a78b4b6fc9895b4e50")?);

            let result = contract_id_to_address("tz1Y68Da76MHixYhJhyU36bVh7a8C9UmtvrR")?;
            assert_eq!(result, hex::decode("00008890efbd6ca6bbd7771c116111a2eec4169e0ed8")?);

            let result = contract_id_to_address("tz2LBtbMMvvguWQupgEmtfjtXy77cHgdr5TE")?;
            assert_eq!(result, hex::decode("0001823dd85cdf26e43689568436e43c20cc7c89dcb4")?);

            let result = contract_id_to_address("tz3e75hU4EhDU3ukyJueh5v6UvEHzGwkg3yC")?;
            assert_eq!(result, hex::decode("0002c2fe98642abd0b7dd4bc0fc42e0a5f7c87ba56fc")?);

            let result = contract_id_to_address("KT1NrjjM791v7cyo6VGy7rrzB3Dg3p1mQki3")?;
            assert_eq!(result, hex::decode("019c96e27f418b5db7c301147b3e941b41bd224fe400")?);

            Ok(())
        }
    }
}

