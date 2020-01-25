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

#[derive(Debug)]
enum Route {
    Bootstrapped,
    CommitHash,
    ActiveChains,
    Protocols,
    ValidBlocks,
    HeadChain,
    ChainsBlockId,
    ContextConstants,
    DevGetBlockEndorsingRights,
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
        (parts.next().unwrap(), parts.next().unwrap())
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
    routes.insert("/chains/:chain_id/blocks/:block_id/context/constants", Route::ContextConstants);
    routes.insert("/chains/:chain_id/blocks/:block_id/helpers/endorsing_rights", Route::DevGetBlockEndorsingRights);
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
        (&Method::GET, Some((Route::DevGetBlockEndorsingRights, params, query))) => {
            let block_id = find_param_value(&params, "block_id").unwrap();
            let level = find_query_value_as_string(&query, "level");
            let cycle = find_query_value_as_string(&query, "cycle");
            let delegate = find_query_value_as_string(&query, "delegate");

            // get RPC response and unpack it from RpcResponseData enum
            match fns::check_and_get_endorsing_rights(block_id, level, cycle, delegate, context_storage, &persistent_storage, state) {
                Ok(Some(RpcResponseData::EndorsingRights(res))) => result_to_json_response(Ok(Some(res)), &log),
                Ok(Some(RpcResponseData::ErrorMsg(res))) => result_to_json_response(Ok(Some(res)), &log),
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
    use std::collections::HashMap;
    use itertools::Itertools;

    use failure::{bail, format_err};
    use hex::FromHex;
    use serde_json::json;

    use crypto::hash::{BlockHash, ChainId, HashType};
    
    use shell::shell_channel::BlockApplied;
    use shell::stats::memory::{Memory, MemoryData, MemoryStatsResult};
    use storage::{BlockHeaderWithHash, BlockMetaStorage, BlockStorage, BlockStorageReader, ContextRecordValue, ContextStorage};
    use storage::block_storage::BlockJsonData;
    use storage::context_storage::ContractAddress;
    use storage::persistent::PersistentStorage;
    use storage::skip_list::Bucket;
    use storage::num_from_slice;
    use tezos_baking::helpers::{cycle_from_level, get_random_number};
    use tezos_context::channel::ContextAction;
    
    use crate::ContextList;
    use crate::encoding::context::ContextConstants;
    use crate::helpers::{FullBlockInfo, PagedResult, RpcResponseData, RpcErrorMsg, EndorsingRight, CycleData};
    use crate::rpc_actor::RpcCollectedStateRef;
    use crate::ts_to_rfc3339;

    /// Retrieve blocks from database.
    pub(crate) fn get_blocks(every_nth_level: Option<i32>, block_id: &str, limit: usize, persistent_storage: &PersistentStorage, state: RpcCollectedStateRef) -> Result<Vec<FullBlockInfo>, failure::Error> {
        let block_storage = BlockStorage::new(persistent_storage);
        let block_hash = block_id_to_block_hash(block_id)?;
        let blocks = match every_nth_level {
            Some(every_nth_level) => block_storage.get_every_nth_with_json_data(every_nth_level, &block_hash, limit),
            None => block_storage.get_multiple_with_json_data(&block_hash, limit),
        }?.into_iter().map(|(header, json_data)| map_header_and_json_to_full_block_info(header, json_data, &state)).collect();
        Ok(blocks)
    }

    /// Get actions for a specific block in ascending order.
    pub(crate) fn get_block_actions(block_id: &str, persistent_storage: &PersistentStorage) -> Result<Vec<ContextAction>, failure::Error> {
        let context_storage = ContextStorage::new(persistent_storage);
        let block_hash = block_id_to_block_hash(block_id)?;
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


    pub(crate) fn check_and_get_endorsing_rights(block_id: &str, input_level: Option<String>, input_cycle: Option<String>, input_delegate: Option<String>, list: ContextList, persistent_storage: &PersistentStorage, state: RpcCollectedStateRef) -> Result<Option< RpcResponseData >, failure::Error> {
        // get level from block id
        let block_level = if let Some(l) = get_level_by_block_id(block_id, list.clone(), persistent_storage)? {
            l as i32
        } else {
            bail!("Level not found for block_id {}", block_id)
        };

        // get the protocol constants from the context
        // TODO: optimalization after merge: input block_level as optional parameter
        let constants = match get_context_constants("main", block_id, list.clone(), persistent_storage)? {
            Some(v) => v,
            None => bail!("Cannot get protocol constants")
        };
        let preserved_cycles = *constants.preserved_cycles() as i32;
        let blocks_per_cycle = *constants.blocks_per_cycle() as i32;
        let current_cycle = cycle_from_level(block_level, blocks_per_cycle);

        //check input cycle
        let cycle: Option<i32> = if let Some(c) = input_cycle {
            let requested_cycle:i32 = c.parse().expect("wrong format of cycle query parameter");
            //check cycle interval
            if (requested_cycle - current_cycle).abs() <= preserved_cycles {
                Some(requested_cycle)
            } else {
                let error_msg = RpcErrorMsg::new(
                    "permanent".to_string(),
                    "proto.005-PsBabyM1.seed.unknown_seed".to_string(), //TODO generate key according protocol
                    None,
                    None,
                    Some((  std::cmp::max(0, current_cycle-preserved_cycles) ).to_string()),
                    Some(requested_cycle.to_string()),
                    Some((current_cycle+preserved_cycles).to_string()),
                );
                return Ok(Some(RpcResponseData::ErrorMsg(error_msg)))
            }
        } else {
            None
        };

        // check if level is in aloved cycle interval
        let level: Option<i32> = if let Some(c) = input_level {
            let requested_level:i32 = c.parse().expect("wrong format of cycle query parameter");
            //check cycle interval
            let requested_cycle = cycle_from_level(requested_level, blocks_per_cycle);
            if (requested_cycle - current_cycle).abs() <= preserved_cycles {
                Some(requested_level)
            } else {
                let error_msg = RpcErrorMsg::new(
                    "permanent".to_string(),
                    "proto.005-PsBabyM1.seed.unknown_seed".to_string(), //TODO generate key according protocol
                    None,
                    None,
                    Some(( std::cmp::max(0, current_cycle-preserved_cycles) ).to_string()),
                    Some(requested_cycle.to_string()),
                    Some((current_cycle+preserved_cycles).to_string()),
                );
                return Ok(Some(RpcResponseData::ErrorMsg(error_msg)))
            }
        } else {
            None
        };

        get_endorsing_rights(block_id, block_level, level, cycle, input_delegate, constants, list, persistent_storage, state)
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

    /// Get information about block
    pub(crate) fn get_full_block(block_id: &str, persistent_storage: &PersistentStorage, state: RpcCollectedStateRef) -> Result<Option<FullBlockInfo>, failure::Error> {
        let block_storage = BlockStorage::new(persistent_storage);
        let block_hash = block_id_to_block_hash(block_id)?;
        let block = block_storage.get_with_json_data(&block_hash)?.map(|(header, json_data)| map_header_and_json_to_full_block_info(header, json_data, &state));

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
            let block_hash = HashType::BlockHash.string_to_bytes(block_id)?;
            let block_meta_storage: BlockMetaStorage = BlockMetaStorage::new(persistent_storage);
            if let Some(block_meta) = block_meta_storage.get(&block_hash)? {
                Some(block_meta.level() as usize)
            } else {
                None
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
            let block_hash = block_id_to_block_hash(block_id)?;
            match block_storage.get(&block_hash)? {
                Some(current_head) => current_head.header.timestamp(),
                None => bail!("block not found in db {}", block_id)
            }
        };
        Ok(timestamp)
    }

    #[inline]
    fn get_context_rollers(level: usize, list: ContextList) -> Result<Option<HashMap<i32, String>>, failure::Error> {
        let mut context_rollers: HashMap<i32, String> = HashMap::new();
        let context_data = {
            let reader = list.read().expect("mutex poisoning");
            if let Ok(Some(c)) = reader.get(level) {
                c
            } else {
                bail!("Context data not found")
            }
        };
        let roll_lists: HashMap<String, Bucket<Vec<u8>>> = context_data.clone().into_iter()
            .filter(|(k, _)| k.contains("roll_list"))
            .collect();
        // take smaller chunks of context data to optimalize key searching
        let owner_data: HashMap<String, Bucket<Vec<u8>>> = context_data.clone().into_iter()
            .filter(|(k, _)| k.contains("data/rolls/owner"))
            .collect();
        let successor_data: HashMap<String, Bucket<Vec<u8>>> = context_data.into_iter()
            .filter(|(k, _)| k.contains("/successor"))
            .collect();

        for roll_key in roll_lists.keys() {
            // get public key address hash from key string
            let public_key: String = match roll_key.split('/').collect::<Vec<_>>().get(9) {
                Some(v) => v.to_string(),
                None => bail!("public key not found in roll list key")
            };

            // get first roll_key
            let mut roll = if let Some(Bucket::Exists(r)) = roll_lists.get(roll_key) {
                r
            } else {
                bail!("empty roll_key")
            };

            // fill roll data and get all successors rolls
            loop {
                let roll_num = num_from_slice!(roll, 0, i32);
                let owner_key = format!("data/rolls/owner/current/{}/{}/{}", roll[3], roll[2], roll_num);

                if let Some(Bucket::Exists(_r)) = owner_data.get(&owner_key) {
                    context_rollers.insert( roll_num.into(), public_key.clone() );
                } else {
                    break;
                }
                
                // get next roll_key
                let successor_key = format!("data/rolls/index/{}/{}/{}/successor", roll[3], roll[2], roll_num);
                if let Some(Bucket::Exists(r)) = successor_data.get(&successor_key) {
                    roll = r
                } else {
                    break;
                }
            }
        }
        Ok(Some(context_rollers))
    }

    #[inline]
    fn get_endorsing_rights(block_id: &str, block_level: i32, input_level: Option<i32>, cycle: Option<i32>, input_delegate: Option<String>, constants: ContextConstants, list: ContextList, persistent_storage: &PersistentStorage, state: RpcCollectedStateRef) -> Result<Option< RpcResponseData >, failure::Error> {
        
        // get requested level or use block head level+1
        let mut is_requested_level = false;
        // output_level is here because of corner case where all levels < 1 are computed as level 1 but oputputed as they are
        // this level is inserted to final endorsing rights
        let mut output_level: i32;
        // first check if level was in query
        let mut requested_level: i32 = if let Some(level) = input_level { 
            is_requested_level=true;
            output_level = level;
            if level < 1 {
                1
            } else {
                level
            }
        } else { //if level not specified the get it by block
            output_level = block_level;
            block_level
        };

        //assign level iterator and assign requested level to correct cycle
        let mut is_cycle = false;
        let cycle_of_requested_level;
        let blocks_per_cycle = *constants.blocks_per_cycle() as i32;
        let level_iterator = if let Some(c) = cycle {
            is_cycle = true;
            is_requested_level = true;
            //first and last level of cycle
            //asign requested level to first level of the cycle to correctly compute snapshot
            requested_level = c * blocks_per_cycle + 1;
            cycle_of_requested_level = c;
            requested_level .. requested_level + blocks_per_cycle
        } else {
            cycle_of_requested_level = cycle_from_level(requested_level, blocks_per_cycle);
            requested_level .. requested_level+1
        };
        println!("begin: block_id:{} level:{} cycle:{} is_cycle:{} requested_level:{}", block_id, requested_level, cycle_of_requested_level, is_cycle, is_requested_level);
        
        let cycle_data = match get_cycle_data(requested_level, cycle_of_requested_level, constants.clone(), list) {
            Ok(d) => d,
            Err(e) => {
                // [{"kind":"permanent","id":"proto.005-PsBabyM1.context.storage_error","missing_key":["cycle","4","random_seed"],"function":"get"}]
                let missing_key = json!(["cycle",format!("{}", cycle_of_requested_level),format!("{}", e)]);
                let error_msg = RpcErrorMsg::new(
                    "permanent".to_string(),
                    "proto.005-PsBabyM1.context.storage_error".to_string(), //TODO generate key according protocol
                    Some(missing_key),
                    Some("get".to_string()),
                    None,
                    None,
                    None
                );
                return Ok(Some(RpcResponseData::ErrorMsg(error_msg)))
            }
        };

        // prepare timestamp computations
        let time_between_blocks: Vec<i64> = constants.time_between_blocks()
            .into_iter()
            .map(|x| x.parse().unwrap())
            .collect();
        // optimalization: if do not need get block header timestamp then do not get it
        let block_timestamp: i64 = if is_cycle || block_level <= requested_level {
            //get block timestamp by block_id un raw epoch format
            get_block_header_timestamp(block_id, &persistent_storage, state)?
        } else {
            0 //dummy value, never used
        };

        // prepare filter by delegate
        let mut check_delegates = false;
        let delegate_filter = if let Some(d) = input_delegate {
            check_delegates = true;
            d
        } else {
            "".to_string() //dummy value, never used
        };

        // byte string for PRNG
        const ENDORSEMENT_USE_STRING: &[u8] = b"level endorsement:";
        let mut endorsing_rights = Vec::<EndorsingRight>::new();
        // when query param cycle is specified then iterate over all cycle levels, else only given level
        for level in level_iterator {
            let timestamp_level = if is_requested_level {
                level - 1
            } else {
                level
            };
            //check if estimated time is computed and convert from raw epoch time to rfc3339 format
            let estimated_time: Option<String> = if block_level <= timestamp_level {
                let est_timestamp = ((timestamp_level - block_level).abs() as i64 * time_between_blocks[0]) + block_timestamp;
                Some(ts_to_rfc3339(est_timestamp))
            } else {
                None
            };

            // endorsement_hash is needed to group all slots by delegate
            let mut endorsement_hash: HashMap<String, Vec<u8>> = HashMap::new();
            let context_rollers = cycle_data.rollers();
            let random_seed = cycle_data.random_seed().to_vec();
            for endorser_slot in (0 .. *constants.endorsers_per_block() as u8).rev() {
                // generate PRNG per endorsement slot and take delegates by roll number from context_rollers
                // if roll number is not found then reroll with new state till roll nuber is found in context_rollers
                let mut state = random_seed.clone();
                loop {
                    let (random_num, sequence) = get_random_number(state, *constants.nonce_length() as usize, blocks_per_cycle, ENDORSEMENT_USE_STRING, level, endorser_slot as i32, *cycle_data.last_roll())?;

                    if let Some(delegate) = context_rollers.get(&random_num) {
                        // collect all slots for each delegate
                        let slots = endorsement_hash.entry(delegate.clone()).or_insert( Vec::new() );
                        slots.push(endorser_slot);
                        break;
                    } else {
                        state = sequence;
                        continue;
                    }
                }
            }

            if is_cycle {
                output_level = level;
            }
            // order descending by delegate address hex byte string
            for delegate in endorsement_hash.keys().sorted().rev() {
                // now is the time to convert delegate address to contract it so we do not convert all addresses only these we need to
                let delegate_contract_id  = address_to_contract_id(delegate)?;
                // filter delegates
                if check_delegates && delegate_contract_id != delegate_filter {
                    continue
                }
                endorsing_rights.push(EndorsingRight::new(output_level, delegate_contract_id, endorsement_hash.get(delegate).unwrap().clone(), estimated_time.clone()))
            }
        }

        Ok(Some(RpcResponseData::EndorsingRights(endorsing_rights)))
    }

    // get cycle data roll_snapshot, random_seed, last_roll and rollers from context list
    // for now there are computed levels to get these data for each key, when ctx list will be fixed these keys should be available in requested_level
    #[inline]
    fn get_cycle_data(requested_level: i32, cycle: i32, constants: ContextConstants, list: ContextList) -> Result<CycleData, failure::Error> {
        // prepare constants
        let blocks_per_cycle = *constants.blocks_per_cycle() as i32;
        let preserved_cycles = *constants.preserved_cycles() as i32;
        let blocks_per_roll_snapshot = *constants.blocks_per_roll_snapshot() as i32;
        
        let mut level_to_get_snapshot_and_seed = requested_level; // this variable will be removed after ctx is fixed
        // the seed and the rolls for the first preserved_cycles listed from 0 are pregenerated and won't change until preserved_cycles + 2
        let mut roll_snapshot: i16 = 0;
        if cycle >= preserved_cycles+2 { //TODO remove condition after ctx is fixed
            let snapshot_key = format!("data/cycle/{}/roll_snapshot", cycle);
            // roll snapshot and random seed is set in last block of requested - preserved_cycles cycle
            // TODO use requested_level after ctx is fixed
            level_to_get_snapshot_and_seed = (cycle - preserved_cycles) * blocks_per_cycle;
            println!("get snapshot_key: level:{} ctx key:{}", level_to_get_snapshot_and_seed, snapshot_key);
            let reader = list.read().expect("mutex poisoning");
            if let Some(Bucket::Exists(data)) = reader.get_key(level_to_get_snapshot_and_seed as usize, &snapshot_key)? {
                roll_snapshot = num_from_slice!(data, 0, i16);
            } else { // key not found
                return Err(format_err!("roll_snapshot"))
            }
        };
        if cycle < preserved_cycles+2 {
            level_to_get_snapshot_and_seed = requested_level;
            println!("setting roll_snapshot to 1 for preserved_cycles+2");
            // for preserved_cycles+2 is snapshot set to 0, this mean that it is index(listed from 0) not order of snapshot(listed from 1) 
            // order of snapshot is needed for algorithm below
            roll_snapshot += 1;
        }

        let random_seed_key = format!("data/cycle/{}/random_seed", cycle);
        println!("get random_seed: level:{} ctx key:{}", level_to_get_snapshot_and_seed, random_seed_key);
        let random_seed = {
            let reader = list.read().expect("mutex poisoning");
            if let Some(Bucket::Exists(data)) = reader.get_key(level_to_get_snapshot_and_seed as usize, &random_seed_key)? {
                data
            } else { // key not found
                return Err(format_err!("random_seed"))
            }
        };

        let snapshot_level;
        if cycle < preserved_cycles+2 {
            snapshot_level = requested_level;
        } else {
            let cycle_of_rolls = cycle - preserved_cycles - 2;
            // last_roll is set in last block of snapshot
            snapshot_level = (cycle_of_rolls * blocks_per_cycle) + ((roll_snapshot as i32) * blocks_per_roll_snapshot);
        };
        // Snapshots of last_roll are listed from 0 and in roll_snapshot is stored order of snapshot.
        // So x last_roll in row have index of roll_snapshot-1
        let last_roll_key = format!("data/cycle/{}/last_roll/{}", cycle, roll_snapshot-1);
        println!("get last_roll: level:{} ctx key:{}", snapshot_level, last_roll_key);
        let last_roll = {
            let reader = list.read().expect("mutex poisoning");
            if let Some(Bucket::Exists(data)) = reader.get_key(snapshot_level as usize, &last_roll_key)? {
                num_from_slice!(data, 0, i32)
            } else { // key not found
                return Err(format_err!("last_roll"))
            }
        };
        
        // get list of rollers from context list
        println!("get_context_rollers: level:{}", snapshot_level);
        let context_rollers = if let Some(rollers) = get_context_rollers(snapshot_level as usize, list.clone())? {
            rollers
        } else {
            return Err(format_err!("rollers"))
        };

        Ok(CycleData::new(
            random_seed,
            last_roll,
            context_rollers
        ))
    }

    #[inline]
    fn address_to_contract_id(address: &str) -> Result<String, failure::Error> {
        if address.len() < 4 {
            bail!("short hash address of contract: {}", address)
        }
        let contract_id = match &address[0..2] {
            "00" => {
                if address.len() < 44 {
                    bail!("short hash address of contract: {}", address)
                }
                match &address[2..4] {
                    "00" => {
                        HashType::ContractTz1Hash.bytes_to_string(&<[u8; 20]>::from_hex(&address[4..44])?)
                    }
                    "01" => {
                        HashType::ContractTz2Hash.bytes_to_string(&<[u8; 20]>::from_hex(&address[4..44])?)
                    }
                    "02" => {
                        HashType::ContractTz3Hash.bytes_to_string(&<[u8; 20]>::from_hex(&address[4..44])?)
                    }
                    _ => bail!("Invalid contract id")
                }
            }
            "01" => {
                if address.len() < 42 {
                    bail!("short hash address of contract: {}", address)
                }
                HashType::ContractKt1Hash.bytes_to_string(&<[u8; 22]>::from_hex(&address[2..42])?)
            }
            _ => bail!("Invalid contract id")
        };
        Ok(contract_id)
    }

    #[inline]
    fn block_id_to_block_hash(block_id: &str) -> Result<BlockHash, failure::Error> {
        let block_hash = HashType::BlockHash.string_to_bytes(block_id)?;
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

