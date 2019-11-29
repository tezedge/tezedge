// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use chrono::prelude::*;
use futures::Future;
use hyper::{Body, Error, Method, Request, Response, Server, StatusCode, Uri};
use hyper::service::{make_service_fn, service_fn};
use path_tree::PathTree;
use riker::actors::ActorSystem;
use slog::{Logger, warn};

use lazy_static::lazy_static;
use shell::shell_channel::BlockApplied;
use tezos_encoding::hash::{BlockHash, HashEncoding, HashType};

use crate::{
    encoding::{base_types::*, monitor::BootstrapInfo}, make_json_response, rpc_actor::RpcServerRef,
    ServiceResult,
    ts_to_rfc3339,
};
use crate::rpc_actor::RpcCollectedStateRef;

#[derive(Debug)]
enum Route {
    Bootstrapped,
    CommitHash,
    ActiveChains,
    Protocols,
    ValidBlocks,
    HeadChain,
    ChainsBlockId,
    // -------------------------- //
    DevGetBlocks,
    DevGetBlockActions,
    StatsMemory,
}

/// Server environment parameters
#[derive(Clone)]
pub(crate) struct RpcServiceEnvironment {
    sys: ActorSystem,
    actor: RpcServerRef,
    db: Arc<rocksdb::DB>,
    genesis_hash: String,
    state: RpcCollectedStateRef,
    log: Logger,
}

impl RpcServiceEnvironment {
    pub fn new(sys: ActorSystem, actor: RpcServerRef, db: Arc<rocksdb::DB>, genesis_hash: &BlockHash, state: RpcCollectedStateRef, log: Logger) -> Self {
        Self { sys, actor, db, genesis_hash: HashEncoding::new(HashType::BlockHash).bytes_to_string(genesis_hash), state, log }
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
            let block = HashEncoding::new(HashType::BlockHash).bytes_to_string(&current_head.hash);
            let timestamp = ts_to_rfc3339(current_head.header.timestamp());
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
fn chains_block_id(chain_id: &str, block_id: &str, state: RpcCollectedStateRef) -> ServiceResult {
    use crate::encoding::chain::BlockInfo;
    if chain_id != "main" || block_id != "head" {
        empty()
    } else {
        let current_head = fns::get_full_current_head(state);
        if let Ok(Some(current_head)) = current_head {
            let resp: BlockInfo = current_head.into();
            make_json_response(&resp)
        } else {
            empty()
        }
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
    routes.insert("/dev/chains/main/blocks", Route::DevGetBlocks);
    routes.insert("/dev/chains/main/blocks/:block_id/actions", Route::DevGetBlockActions);
    routes.insert("/stats/memory", Route::StatsMemory);
    routes
}

/// Simple endpoint routing handler
async fn router(req: Request<Body>, env: RpcServiceEnvironment) -> ServiceResult {
    let RpcServiceEnvironment { sys, actor, db, log, genesis_hash, state } = env;

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
        (&Method::GET, Some((Route::HeadChain, params, _))) => {
            let chain_id = find_param_value(&params, "chain_id").unwrap();
            head_chain(chain_id, state)
        }
        (&Method::GET, Some((Route::ChainsBlockId, params, _))) => {
            let chain_id = find_param_value(&params, "chain_id").unwrap();
            let block_id = find_param_value(&params, "block_id").unwrap();
            chains_block_id(chain_id, block_id, state)
        }
        (&Method::GET, Some((Route::DevGetBlocks, _, query))) => {
            let from_block_id = unwrap_block_hash(find_query_value_as_string(&query, "from_block_id"), state, genesis_hash);
            let limit = find_query_value_as_usize(&query, "limit").unwrap_or(50);
            result_to_json_response(fns::get_blocks(from_block_id, limit, db), &log)
        }
        (&Method::GET, Some((Route::DevGetBlockActions, params, _))) => {
            let block_id = find_param_value(&params, "block_id").unwrap();
            result_to_json_response(fns::get_block_actions(block_id, db), &log)
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

/// Unwraps a block hash or provides alternative block hash.
/// Alternatives are: genesis block or current head
fn unwrap_block_hash(block_id: Option<String>, state: RpcCollectedStateRef, genesis_hash: String) -> String {
    block_id.unwrap_or_else(|| {
        let state = state.read().unwrap();
        state.current_head().as_ref()
            .map(|current_head| HashEncoding::new(HashType::BlockHash).bytes_to_string(&current_head.hash))
            .unwrap_or(genesis_hash)
    })
}


/// This submodule contains service functions implementation.
mod fns {
    use std::sync::Arc;

    use storage::{BlockMetaStorage, BlockStorage, BlockStorageReader, ContextStorage};
    use tezos_context::channel::ContextAction;
    use tezos_encoding::hash::{HashEncoding, HashType};
    use shell::stats::memory::{Memory, MemoryData, MemoryStatsResult};

    use crate::helpers::FullBlockInfo;
    use crate::rpc_actor::RpcCollectedStateRef;

    /// Retrieve blocks from database.
    pub(crate) fn get_blocks(block_id: String, limit: usize, db: Arc<rocksdb::DB>) -> Result<Vec<FullBlockInfo>, failure::Error> {
        let block_storage = BlockStorage::new(db.clone());
        let block_meta_storage = BlockMetaStorage::new(db);

        let mut resp_data = Vec::with_capacity(limit);
        // get starting block hash or use genesis hash
        let mut block_hash = HashEncoding::new(HashType::BlockHash).string_to_bytes(&block_id)?;

        for _ in 0..limit {
            match block_meta_storage.get(&block_hash)? {
                Some(meta) => match block_storage.get(&block_hash)? {
                    Some(block) => {
                        resp_data.push(block.into());
                        match meta.predecessor {
                            Some(predecessor) => if block_hash != predecessor {
                                block_hash = predecessor
                            } else {
                                // found genesis
                                break
                            },
                            None => break
                        }
                    }
                    None => break
                }
                None => break
            }
        }

        Ok(resp_data)
    }

    /// Get actions for a specific block in ascending order.
    pub(crate) fn get_block_actions(block_id: &str, db: Arc<rocksdb::DB>) -> Result<Vec<ContextAction>, failure::Error> {
        let context_storage = ContextStorage::new(db);

        let block_hash = HashEncoding::new(HashType::BlockHash).string_to_bytes(block_id)?;
        context_storage.get_by_block_hash(&block_hash)
            .map(|values| values.into_iter().map(|v| v.action).collect())
            .map_err(|e| e.into())
    }

    /// Get information about current head
    pub(crate) fn get_full_current_head(state: RpcCollectedStateRef) -> Result<Option<FullBlockInfo>, failure::Error> {
        let state = state.read().unwrap();
        let current_head = state.current_head().as_ref().map(|current_head| {
            let mut head: FullBlockInfo = current_head.clone().into();
            head.chain_id = HashEncoding::new(HashType::ChainId).bytes_to_string(state.chain_id());
            head
        });

        Ok(current_head)
    }

    pub(crate) fn get_stats_memory() -> MemoryStatsResult<MemoryData> {
        let memory = Memory::new();
        memory.get_memory_stats()
    }

}

