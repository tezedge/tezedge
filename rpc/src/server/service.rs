// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;
use std::net::SocketAddr;

use chrono::prelude::*;
use futures::Future;
use hyper::{Body, Error, Method, Request, Response, Server, StatusCode};
use hyper::service::{make_service_fn, service_fn};
use path_tree::PathTree;
use riker::actors::ActorSystem;

use lazy_static::lazy_static;
use shell::shell_channel::BlockApplied;
use tezos_encoding::hash::{HashEncoding, HashType};

use crate::{
    encoding::{base_types::*, monitor::BootstrapInfo}, make_json_response, rpc_actor::RpcServerRef,
    server::{ask::ask, control_msg::*},
    ServiceResult,
    ts_to_rfc3339,
};
use crate::server::control_msg::GetBlocks;

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
}

/// Spawn new HTTP server on given address interacting with specific actor system
pub fn spawn_server(addr: &SocketAddr, sys: ActorSystem, actor: RpcServerRef) -> impl Future<Output=Result<(), Error>> {
    Server::bind(addr)
        .serve(make_service_fn(move |_| {
            let sys = sys.clone();
            let actor = actor.clone();
            async move {
                let sys = sys.clone();
                let actor = actor.clone();
                Ok::<_, Error>(service_fn(move |req| {
                    let sys = sys.clone();
                    let actor = actor.clone();
                    async move {
                        router(req, sys, actor).await
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
async fn bootstrapped(sys: ActorSystem, actor: RpcServerRef) -> ServiceResult {
    let current_head = ask(&sys, &actor, GetCurrentHead::Request).await;
    loop {
        if let GetCurrentHead::Response(current_head) = current_head {
            let resp = if current_head.is_some() {
                let current_head: BlockApplied = current_head.unwrap();
                let block = HashEncoding::new(HashType::BlockHash).bytes_to_string(&current_head.hash);
                let timestamp = ts_to_rfc3339(current_head.header.timestamp());
                BootstrapInfo::new(block.into(), TimeStamp::Rfc(timestamp))
            } else {
                BootstrapInfo::new(String::new().into(), TimeStamp::Integral(0))
            };
            return make_json_response(&resp);
        } else {
            tokio::timer::delay_for(std::time::Duration::from_secs(1)).await
        }
    }
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

async fn dev_get_blocks(sys: ActorSystem, actor: RpcServerRef, from_block_id: Option<String>, limit: usize) -> ServiceResult {
    match ask(&sys, &actor, GetBlocks::Request { block_hash: from_block_id, limit }).await {
        GetBlocks::Response(blocks) => {
            make_json_response(&blocks)
        }
        _ => empty()
    }
}

async fn head_chain(sys: ActorSystem, actor: RpcServerRef, chain_id: &str, _next_protocol: Vec<String>) -> ServiceResult {
    if chain_id == "main" {
        let current_head = ask(&sys, &actor, GetFullCurrentHead::Request).await;
        if let GetFullCurrentHead::Response(Some(_current_head)) = current_head {
            empty()
        } else {
            empty()
        }
    } else {
        empty()
    }
}
/// GET /chains/<chain_id>/blocks/<block_id> endpoint handler
async fn chains_block_id(sys: ActorSystem, actor: RpcServerRef, chain_id: &str, block_id: &str) -> ServiceResult {
    use crate::encoding::chain::BlockInfo;
    if chain_id != "main" || block_id != "head" {
        empty()
    } else {
        let current_head: GetFullCurrentHead = ask(&sys, &actor, GetFullCurrentHead::Request).await;
        if let GetFullCurrentHead::Response(Some(current_head)) = current_head {
            let resp: BlockInfo = current_head.into();
            make_json_response(&resp)
        } else {
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
    routes
}

/// Simple endpoint routing handler
async fn router(req: Request<Body>, sys: ActorSystem, actor: RpcServerRef) -> ServiceResult {
    match (req.method(), ROUTES.find(req.uri().path())) {
        (&Method::GET, Some((Route::Bootstrapped, _))) => bootstrapped(sys, actor).await,
        (&Method::GET, Some((Route::CommitHash, _))) => commit_hash(sys, actor).await,
        (&Method::GET, Some((Route::ActiveChains, _))) => active_chains(sys, actor).await,
        (&Method::GET, Some((Route::Protocols, _))) => protocols(sys, actor).await,
        (&Method::GET, Some((Route::ValidBlocks, _))) => {
            let mut protocol = Vec::new();
            let mut next_protocol = Vec::new();
            let mut chain = Vec::new();

            req.uri().query()
                .map(parse_query_string)
                .map(|query_parts| query_parts.iter().for_each(|(&key, values)| {
                    match key {
                        "protocol" => protocol.extend(values.iter().map(|value| value.to_string())),
                        "next_protocol" => next_protocol.extend(values.iter().map(|value| value.to_string())),
                        "chain" => chain.extend(values.iter().map(|value| value.to_string().into())),
                        _ => ()
                    }
                }));
            valid_blocks(sys, actor, protocol, next_protocol, chain).await
        }
        (&Method::GET, Some((Route::HeadChain, params))) => {
            let chain_id = find_param_value(&params, "chain_id").unwrap();
            let mut next_protocol = Vec::new();
            req.uri().query()
                .map(parse_query_string)
                .map(|query_parts| query_parts.iter().for_each(|(&key, values)| {
                    match key {
                        "next_protocol" => next_protocol.extend(values.iter().map(|value| value.to_string())),
                        _ => ()
                    }
                }));
            head_chain(sys, actor, chain_id, next_protocol).await
        }
        (&Method::GET, Some((Route::ChainsBlockId, params))) => {
            let chain_id = find_param_value(&params, "chain_id").unwrap();
            let block_id = find_param_value(&params, "block_id").unwrap();
            chains_block_id(sys, actor, chain_id, block_id).await
        }
        (&Method::GET, Some((Route::DevGetBlocks, _))) => {
            let (from_block_id, limit) = req.uri().query()
                .map(parse_query_string)
                .map(|query_parts| {
                    let from_block_id = query_parts.get("from_block_id")
                        .and_then(|values| values.first())
                        .map(|value| value.to_string());
                    let limit = query_parts.get("limit")
                        .and_then(|values| values.first())
                        .and_then(|value| value.parse::<usize>().ok())
                        .filter(|value| *value < 1_000)
                        .unwrap_or(50);
                    (from_block_id, limit)
                }).unwrap_or((None, 50));

            dev_get_blocks(sys, actor, from_block_id, limit).await
        }
        _ => not_found()
    }
}