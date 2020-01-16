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
use storage::persistent::{PersistentStorage, ContextList};

use crate::{
    encoding::{base_types::*, monitor::BootstrapInfo}, make_json_response, rpc_actor::RpcServerRef,
    ServiceResult,
    ts_to_rfc3339,
};
use crate::rpc_actor::RpcCollectedStateRef;

use crate::helpers::BakingRights;

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
    ChainsBlockIdBakingRights,
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
async fn baking_rights(block_id: &str, delegate: Option<String>, level: &Option<String>, cycle: &String, max_priority: String, has_all: bool, state: RpcCollectedStateRef, persistent_storage: &PersistentStorage, list: ContextList, _log: &Logger) -> ServiceResult {
    let state_read = state.read().unwrap();

    let baking_rights_info = match state_read.current_head().as_ref() {
        Some(current_head) => {
            let current_head: BlockApplied = current_head.clone();
            let timestamp = ts_to_rfc3339(current_head.header().header.timestamp());
            let head_level = current_head.header().header.level();
            fns::get_baking_rights(block_id, delegate, level, cycle, max_priority, has_all, head_level.into(), timestamp, list, persistent_storage).unwrap().unwrap()
        }
        None => vec![BakingRights::new(0, String::new().into(), 0, String::new().into())]
    };
    make_json_response(&baking_rights_info)
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
    routes.insert("/chains/:chain_id/blocks/:block_id/helpers/baking_rights", Route::ChainsBlockIdBakingRights);
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
        (&Method::GET, Some((Route::ChainsBlockIdBakingRights, params, query))) => {
            let block_id = find_param_value(&params, "block_id").unwrap();
            let max_priority = find_query_value_as_string(&query, "max_priority").unwrap_or("64".to_string());
            let level = find_query_value_as_string(&query, "level");
            let delegate = find_query_value_as_string(&query, "delegate");
            let cycle = find_query_value_as_string(&query, "cycle").unwrap_or("".to_string());
            let has_all = query.contains_key("all");

            //warn!(log, "Args = Block Id: {:?} max_priority: {:?} level: {:?} delegate: {:?} cycle {:?}", &block_id, &max_priority, level.clone().unwrap_or("".to_string()), delegate.clone().unwrap_or("".to_string()), &cycle);

            baking_rights(block_id, delegate, &level, &cycle, max_priority, has_all, state, &persistent_storage, context_storage, &log).await
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

    use failure::bail;

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
    use crate::helpers::{FullBlockInfo, PagedResult};
    use crate::rpc_actor::RpcCollectedStateRef;

    use hex::FromHex;
    use chrono::{DateTime, Duration};
    use chrono::prelude::*;
    use crate::helpers::BakingRights;
    use rand::Rng;

    macro_rules! merge_slices {
        ( $($x:expr),* ) => {{
            let mut res = vec![];
            $(
                res.extend_from_slice($x);
            )*
            res
        }}
    }

    macro_rules! num_from_slice_le {
        ($buf:expr, $from_idx:expr, $num:ident) => {{
            let mut bytes: [u8; std::mem::size_of::<$num>()] = Default::default();
            bytes.copy_from_slice(&$buf[$from_idx .. $from_idx + std::mem::size_of::<$num>()]);
            $num::from_le_bytes(bytes)
        }}
    }

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
    pub(crate) fn get_block_level(block_id: &str, persistent_storage: &PersistentStorage, list: &ContextList) -> Result<Option<usize>, failure::Error> {
        if block_id == "head" {
            let reader = list.read().expect("mutex poisoning");
            Ok(Some(reader.len() - 1))
        } else {
            let block_hash = HashType::BlockHash.string_to_bytes(block_id)?;
            let block_meta_storage: BlockMetaStorage = BlockMetaStorage::new(persistent_storage);
            if let Some(block_meta) = block_meta_storage.get(&block_hash)? {
                Ok(Some(block_meta.level() as usize))
            } else {
                Ok(None)
            }
        }
    }

    pub(crate) fn get_rolls(level: usize, _cycle: &String, _persistent_storage: &PersistentStorage, list: ContextList) -> Result<Option<HashMap<i64, String>>, failure::Error> {
        const ROLL_NUM_START: usize = 0;
        const ADDRESS_POSITION: usize = 9;

        // get the level of from the block
        // let level = match get_block_level(block_id, persistent_storage, &list)? {
        //     Some(v) => v,
        //     None => return Ok(None)  // Cannot get level
        // };

        // TODO: implement getting the random seed out of context storage
        //       based on the cycle 

        // get the whole context
        let context = {
            let reader = list.read().unwrap();
            if let Ok(Some(ctx)) = reader.get(level) {
                ctx
            } else {
                bail!("Context not found")
            }
        };

        let roll_data = context.clone();
        // get all the relevant data out of the context database
        let data: HashMap<String, Bucket<Vec<u8>>> = context.into_iter()
            .filter(|(k, _)| k.contains("data/rolls/owner/current") || k.contains("/successor") || k.contains("/random_seed"))
            .collect();

        // get the roll_lists out of the context storage
        // roll_lists are the beginning of a linked list of the rolls pointing to the data/rolls/owner/current/*/*/<first_roll>
        // This seems a little redundant... but with the above solution we only traverse the whole context only once
        let roll_lists: HashMap<String, Bucket<Vec<u8>>> = roll_data.into_iter()
            .filter(|(k, _)| k.contains("roll_list"))
            .collect();
            
        let mut roll_owners: HashMap<i64, String> = HashMap::new();

        for key in roll_lists.keys() {
            // extract the delegate address (public key hash <pkh>) from the key
            // let public_key_hash: String = key.split('/').collect::<Vec<_>>()[9].to_string(); //[9]
            let public_key_hash: String = match key.split('/').collect::<Vec<_>>().get(ADDRESS_POSITION) {
                Some(v) => v.to_string(),
                None => bail!("Address not found in key")
            };

            let contract_id = address_to_contract_id(&public_key_hash)?;

            // get the first roll
            let mut next_roll;
            if let Some(Bucket::Exists(roll)) = roll_lists.get(key) {
                next_roll = roll;
            } else {
                bail!("No first roll for the delegate")
            }

            // fill out the roll_owners hash map using the linked list from the context storage
            loop {
                let roll_num = num_from_slice!(next_roll, ROLL_NUM_START, i32);

                // construct the whole owner/successor key
                // /<rol_num little-endian 1st byte>/<roll_num little-endian 2nd byte>/<roll_num in decimal>
                // Note: this will work up to 0xffff rolls, not sure how exactly they implement rolls over this number with this key format
                let owner_key = format!("data/rolls/owner/current/{}/{}/{}", next_roll[3], next_roll[2], roll_num);
                let successor_key = format!("data/rolls/index/{}/{}/{}/successor", next_roll[3], next_roll[2], roll_num);
            
                if let Some(Bucket::Exists(_roll)) = data.get(&owner_key) {
                    roll_owners.insert(roll_num.into(), contract_id.clone());
                } else {
                    bail!("Roll owner key not found for key: {}", &owner_key)
                }

                // get the next roll for the delegate
                if let Some(Bucket::Exists(roll)) = data.get(&successor_key) {
                    next_roll = roll
                } else {
                    break;
                }
            }
        }
        Ok(Some(roll_owners))
    }

    pub(crate) fn get_baking_rights(block_id: &str, delegate: Option<String>, level: &Option<String>, cycle: &String, max_priority: String, has_all: bool, head_level: i64, timestamp: String, list: ContextList, persistent_storage: &PersistentStorage) -> Result<Option<Vec<BakingRights>>, failure::Error> {
        const NO_CYCLE_FLAG: i64 = -1;
        let mut baking_rights = Vec::<BakingRights>::new();

        // get the protocol constants from the context
        let constants = match get_context_constants("main", block_id, list.clone(), persistent_storage)? {
            Some(v) => v,
            None => bail!("Cannot get protocol constants")
        };

        // time between blocks values. String -> i64
        let time_between_blocks: Vec<i64> = constants.time_between_blocks()
            .into_iter()
            .map(|x| x.parse().unwrap())
            .collect();
        
        // level of the block specified in the url params
        let block_level = match get_block_level(block_id, persistent_storage, &list)? {
            Some(v) => v as i64,
            None => bail!("Cannot get level")  // Cannot get level
        };

        let requested_cycle: i64 = match cycle.parse() {
            // cycle requested from query
            Ok(cycle) => {
                // check whether the cycle is reachable
                // the storage stores cycle up to: current_cycle + perserved_cycles(5) + 2
                // e.g. if head is in cycle 66, we can get the rights for up to cycle 73
                if ((cycle as i64 - (block_level / constants.blocks_per_cycle())).abs() as i64) <= constants.preserved_cycles() + 2 {
                    cycle
                } else {
                    bail!("Cycle not found in block: {}", block_id)
                }
            },
            // no cycle requested
            Err(_e) => NO_CYCLE_FLAG
        };

        // If no level is specified, we return the next block to be baked
        let mut requested_level: i64 = match level {
            Some(level) => level.parse()?,
            None => block_level + 1,
        };

        // if a cycle is specified, we need to iterate through all of the levels in the cycle
        let level_to_itarate_to = if requested_cycle >= 0 {
            requested_level = requested_cycle * constants.blocks_per_cycle(); // first block of the cycle
            requested_level + constants.blocks_per_cycle()
        } else {
            requested_level
        };

        // get the requested delegate from the query, set to all if not specified
        let requested_delegate = match delegate {
            Some(delegate) => delegate,
            None => "all".to_string() // all
        };
        // random number generator
        // let mut rng = rand::thread_rng();

        let cycle_of_requested_level = requested_level / constants.blocks_per_cycle();
        let snapshot_key = format!("data/cycle/{}/roll_snapshot", cycle_of_requested_level);
        println!("{}", snapshot_key);
        let roll_snapshot;
        {
            let reader = list.read().unwrap();
            if let Some(Bucket::Exists(data)) = reader.get_key(requested_level as usize, &snapshot_key)? {
                roll_snapshot = num_from_slice!(data, 0, i16);
            } else {
                roll_snapshot = Default::default();
            }
        }

        let last_roll_key = format!("data/cycle/{}/last_roll/{}", cycle_of_requested_level, roll_snapshot);
        println!("{}", last_roll_key);

        let last_roll;
        {
            let reader = list.read().unwrap();
            if let Some(Bucket::Exists(data)) = reader.get_key(requested_level as usize, &last_roll_key)? {
                last_roll = num_from_slice!(data, 0, i32);
            } else {
                last_roll = Default::default();
            }
        }
        println!("Last roll: {}", last_roll);

        let random_seed_key = format!("data/cycle/{}/random_seed", cycle_of_requested_level);
        println!("{}", random_seed_key);
        let random_seed;
        {
            let reader = list.read().unwrap();
            if let Some(Bucket::Exists(data)) = reader.get_key(requested_level as usize, &random_seed_key)? {
                random_seed = data;
            } else {
                random_seed = Default::default();
            }
        }
        
        let mut snapshot_level;

        if cycle_of_requested_level <= *constants.preserved_cycles() {
            snapshot_level = head_level;
        } else {
            snapshot_level = ((cycle_of_requested_level - constants.preserved_cycles()) * constants.blocks_per_cycle()) + ((roll_snapshot + 1) as i64 * constants.blocks_per_roll_snapshot());
        };

        println!("Snapshot level: {}", snapshot_level);
        // get the rolls from the context storage
        let rolls = get_rolls(snapshot_level as usize, &cycle, persistent_storage, list)?;
        let roll_owners = match rolls {
            Some(r) => r,
            None => bail!("Error getting rolls")
        };

        // iterate through the whole cycle if necessery
        for level in requested_level..(level_to_itarate_to + 1) {
            // 30 is from the protocol constants
            let seconds_to_add = (level - head_level).abs() * time_between_blocks[0];
            let mut estimated_timestamp: Option<DateTime<_>>;
            estimated_timestamp = Some(DateTime::parse_from_rfc3339(&timestamp).unwrap().checked_add_signed(Duration::seconds(seconds_to_add.into())).unwrap());
            
            // as we assign the roles, the default behavior is to include only the top priority for the delegate
            // we define a hashset to keep track of the delegates with priorities allready assigned
            let mut assigned = HashSet::new();
            for priority in 0..max_priority.parse()? {
                // draw the rolls for the requested parameters
                // Note: this is a temporary solution, we should replace it with the tezos PRNG
                // It will utilize the random_seed found in the context storage and the cycle_position of the
                // block to be baked
                // let delegate_to_assign = match roll_owners.get(&rng.gen_range(0, roll_owners.len() as i64)) {
                let randomed = get_random_number(&random_seed, *constants.nonce_length() as usize, level as i32, priority, last_roll)?;
                println!("RANDOMMED ROLL: {}", randomed);
                println!("");
                let delegate_to_assign = match roll_owners.get(&randomed) {
                    Some(d) => d,
                    None => bail!("Roll not found")
                };
                
                // if the delegate was assgined and the the has_all flag is not set skip this priority
                if assigned.contains(delegate_to_assign) && !has_all {
                    continue;
                }

                // we omit the estimated_time field if the block on the requested level is allready baked
                if head_level <= level {
                    baking_rights.push(BakingRights::new(level, delegate_to_assign.to_string(), priority.into(), Some(estimated_timestamp.unwrap().to_rfc3339_opts(SecondsFormat::Secs, true))));
                    estimated_timestamp = Some(estimated_timestamp.unwrap().checked_add_signed(Duration::seconds(time_between_blocks[1])).unwrap());
                } else {
                    baking_rights.push(BakingRights::new(level, delegate_to_assign.to_string(), priority.into(), None))
                }
                assigned.insert(delegate_to_assign);
            }
        }
        // if there is no delegate specified, retrive all the delegates 
        if requested_delegate == "all" {
            Ok(Some(baking_rights))
        } else {
            Ok(Some(baking_rights.into_iter().filter(|val| val.delegate().contains(&requested_delegate)).collect::<Vec<BakingRights>>()))
        }
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
        let level = if block_id == "head" {
            let reader = list.read().expect("mutex poisoning");
            reader.len() - 1
        } else {
            let block_hash = HashType::BlockHash.string_to_bytes(block_id)?;
            let block_meta_storage: BlockMetaStorage = BlockMetaStorage::new(persistent_storage);
            if let Some(block_meta) = block_meta_storage.get(&block_hash)? {
                block_meta.level() as usize
            } else {
                return Ok(None);
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

    // tezos PRNG
    //TODO: should create a module for this
    pub(crate) fn get_random_number(random_seed: &[u8], nonce_size: usize, level: i32, offset: i32, bound: i32) -> Result<i64, failure::Error> {
        let zero_bytes: Vec<u8> = vec![0; 32];
        // Note: use only the baking variant for now
        const BAKING_STRING: &[u8] = b"level baking:";

        let cycle_position = level % 2048; // TODO: use from constants

        println!("Offset: {:?}", offset);
        println!("Bound: {:?}", bound);
        println!("Level: {:?}", level);
        println!("Cycle position: {:?}", &cycle_position);
        println!("Use string: {:?}", &BAKING_STRING);

        // with zero bytes
        println!("Initial vec: {:?}", merge_slices!(random_seed, &zero_bytes, BAKING_STRING, &cycle_position.to_be_bytes()));
        let rd = blake2b::digest_256(&merge_slices!(random_seed, &zero_bytes, BAKING_STRING, &cycle_position.to_be_bytes())).to_vec();

        println!("Rd: {:?}", rd);
        
        let higher = num_from_slice!(rd, 0, i32) ^ offset;
        println!("HIGHER BYTES: {:?}", &higher.to_be_bytes());
        println!("Rd after mod: {:?}", merge_slices!(&higher.to_be_bytes(), &rd[4..]));
        
        let mut sequence = blake2b::digest_256(&merge_slices!(&higher.to_be_bytes(), &rd[4..])).to_vec();
        println!("Sequence: {:?}", sequence);
        let v: i32;
        // Note: we can use while here most likely, using loop to be semanticly similar to the ocaml version
        loop {
            let hashed = blake2b::digest_256(&sequence).to_vec();
            let drop_if_over = i32::max_value() - (i32::max_value() % bound);

            let r = num_from_slice!(hashed, 0, i32).abs();

            if r >= drop_if_over {
                sequence = hashed;
                continue;
            } else {
                v = r % bound;
                break;
            };
        }
        Ok(v.into())
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
    fn address_to_contract_id(address: &str) -> Result<String, failure::Error> {

        let contract_id = match &address[0..2] {
            "00" => {
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
                HashType::ContractKt1Hash.bytes_to_string(&<[u8; 22]>::from_hex(&address[2..42])?)
            }
            _ => bail!("Invalid contract id")
        };
        Ok(contract_id)
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

