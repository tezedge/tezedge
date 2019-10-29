use hyper::{Body, Response, Error, Server, Request, StatusCode, Method};
use hyper::service::{service_fn, make_service_fn};
use std::net::SocketAddr;
use futures::Future;
use crate::rpc_actor::RpcServerRef;
use riker::actors::ActorSystem;
use crate::server::ask::ask;
use serde_json;
use chrono::prelude::*;
use crate::encoding::base_types::*;
use std::collections::HashMap;
use lazy_static::lazy_static;
use regex::Regex;
use tezos_encoding::hash::{HashEncoding, HashType};
use crate::encoding::monitor::BootstrapInfo;

type ServiceResult = Result<Response<Body>, Box<dyn std::error::Error + Sync + Send>>;

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

fn ts_to_rfc3339(ts: i64) -> String {
    Utc.from_utc_datetime(&NaiveDateTime::from_timestamp(ts, 0))
        .to_rfc3339_opts(SecondsFormat::Secs, true)
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
/// Functions takes URI query in format key1=val1&key1=val2&key2=val3
/// and produces map { key1: [val1, val2], key2: [val3] }
fn parse_queries(query: &str) -> HashMap<&str, Vec<&str>> {
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

/// GET /monitor/bootstrapped endpoint handler
async fn bootstrapped(sys: ActorSystem, actor: RpcServerRef) -> ServiceResult {
    use crate::server::control_msg::GetCurrentHead;
    use shell::shell_channel::BlockApplied;

    let current_head = ask(&sys, &actor, GetCurrentHead::Request).await;
    if let GetCurrentHead::Response(current_head) = current_head {
        let resp = serde_json::to_string(&if current_head.is_some() {
            let current_head: BlockApplied = current_head.unwrap();
            let block = HashEncoding::new(HashType::BlockHash).bytes_to_string(&current_head.hash);
            let timestamp = ts_to_rfc3339(current_head.header.timestamp());
            BootstrapInfo::new(block.into(), TimeStamp::Rfc(timestamp))
        } else {
            BootstrapInfo::new(String::new().into(), TimeStamp::Integral(0))
        })?;
        Ok(Response::new(Body::from(resp)))
    } else {
        empty()
    }
}

/// GET /monitor/commit_hash endpoint handler
async fn commit_hash(_sys: ActorSystem, _actor: RpcServerRef) -> ServiceResult {
    let resp = serde_json::to_string(&UniString::from(env!("GIT_HASH")))?;
    Ok(Response::new(Body::from(resp)))
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

async fn head_chain(_sys: ActorSystem, _actor: RpcServerRef, _chain_id: &str, _next_protocol: Vec<String>) -> ServiceResult {
    empty()
}

lazy_static! {
    static ref HEADS_CHAIN: Regex = Regex::new(r"/monitor/heads/(?P<chain_id>\w+)").expect("Invalid regex");
}

/// Simple endpoint routing handler
async fn router(req: Request<Body>, sys: ActorSystem, actor: RpcServerRef) -> ServiceResult {
    match (req.method(), req.uri().path()) {
        (&Method::GET, "/monitor/bootstrapped") => bootstrapped(sys, actor).await,
        (&Method::GET, "/monitor/commit_hash") => commit_hash(sys, actor).await,
        (&Method::GET, "/monitor/active_chains") => active_chains(sys, actor).await,
        (&Method::GET, "/monitor/protocols") => protocols(sys, actor).await,
        (&Method::GET, "/monitor/valid_blocks") => {
            let mut protocol: Vec<String> = Vec::new();
            let mut next_protocol: Vec<String> = Vec::new();
            let mut chain: Vec<UniString> = Vec::new();
            if let Some(query) = req.uri().query() {
                let parts = parse_queries(query);
                if let Some(protocols) = parts.get("protocol") {
                    for proto in protocols {
                        protocol.push(proto.to_string());
                    }
                }
                if let Some(next_protocols) = parts.get("next_protocol") {
                    for next in next_protocols {
                        next_protocol.push(next.to_string());
                    }
                }
                if let Some(chains) = parts.get("chain") {
                    for c in chains {
                        chain.push(c.to_string().into());
                    }
                }
            }
            valid_blocks(sys, actor, protocol, next_protocol, chain).await
        }
        _ => {
            // We still need to go through pattern, for URIs with wildcart parts
            if req.method() == Method::GET {
                if let Some(captures) = HEADS_CHAIN.captures(req.uri().path()) {
                    let chain_id = &captures["chain_id"];
                    let mut next_protocol: Vec<String> = Vec::new();
                    if let Some(query) = req.uri().query() {
                        let parts = parse_queries(query);
                        if let Some(protos) = parts.get("next_protocol") {
                            for proto in protos {
                                next_protocol.push(proto.to_string());
                            }
                        }
                    }
                    head_chain(sys, actor, chain_id, next_protocol).await
                } else {
                    not_found()
                }
            } else {
                not_found()
            }
        }
    }
}