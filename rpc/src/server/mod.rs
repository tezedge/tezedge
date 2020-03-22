// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;

use getset::Getters;
use hyper::{Body, Request, Response};
use hyper::service::{make_service_fn, service_fn};
use riker::actors::ActorSystem;
use slog::Logger;

use crypto::hash::{BlockHash, HashType};
use storage::persistent::PersistentStorage;

use crate::empty;
use crate::rpc_actor::{RpcCollectedStateRef, RpcServerRef};

mod handler;
mod dev_handler;
mod service;
mod router;

/// Server environment parameters
#[derive(Getters, Clone)]
pub struct RpcServiceEnvironment {
    #[get = "pub(crate)"]
    sys: ActorSystem,
    #[get = "pub(crate)"]
    actor: RpcServerRef,
    #[get = "pub(crate)"]
    persistent_storage: PersistentStorage,
    #[get = "pub(crate)"]
    genesis_hash: String,
    #[get = "pub(crate)"]
    state: RpcCollectedStateRef,
    #[get = "pub(crate)"]
    log: Logger,
}

impl RpcServiceEnvironment {
    pub fn new(sys: ActorSystem, actor: RpcServerRef, persistent_storage: &PersistentStorage, genesis_hash: &BlockHash, state: RpcCollectedStateRef, log: Logger) -> Self {
        Self { sys, actor, persistent_storage: persistent_storage.clone(), genesis_hash: HashType::BlockHash.bytes_to_string(genesis_hash), state, log }
    }
}

pub type Params = Vec<(String, String)>;

pub type Query = HashMap<String, Vec<String>>;

pub type HResult = Result<Response<Body>, Box<dyn std::error::Error + Sync + Send>>;

pub type Handler = Arc<dyn Fn(Request<Body>, Params, Query, RpcServiceEnvironment) -> Box<dyn Future<Output = HResult> + Send> + Send + Sync>;


/// Spawn new HTTP server on given address interacting with specific actor system
pub fn spawn_server(bind_address: &SocketAddr, env: RpcServiceEnvironment) -> impl Future<Output=Result<(), hyper::Error>> {
    let routes = Arc::new(router::create_routes());

    hyper::Server::bind(bind_address)
        .serve(make_service_fn(move |_| {
            let env = env.clone();
            let routes = routes.clone();

            async move {
                let env = env.clone();
                let routes = routes.clone();
                Ok::<_, hyper::Error>(service_fn(move |req: Request<Body>| {
                    let env = env.clone();
                    let routes = routes.clone();
                    async move {
                        if let Some((handler, params)) = routes.find(&req.uri().path().to_string()) {
                            let params: Params = params.into_iter().map(|(param, value)| (param.to_string(), value.to_string())).collect();
                            let query: Query = req.uri().query().map(parse_query_string).unwrap_or_else(|| HashMap::new());

                            let handler = handler.clone();
                            let fut = handler(req, params, query, env);
                            Pin::from(fut).await
                        } else {
                            empty()
                        }
                    }
                }))
            }
        }))
}

/// Helper for parsing URI queries.
/// Functions takes URI query in format `key1=val1&key1=val2&key2=val3`
/// and produces map `{ key1: [val1, val2], key2: [val3] }`
fn parse_query_string(query: &str) -> HashMap<String, Vec<String>> {
    let mut ret: HashMap<String, Vec<String>> = HashMap::new();
    for (key, value) in query.split('&').map(|x| {
        let mut parts = x.split('=');
        (parts.next().unwrap(), parts.next().unwrap_or(""))
    }) {
        if let Some(vals) = ret.get_mut(key) {
            // append value to existing vector
            vals.push(value.to_string());
        } else {
            // create new vector with a single value
            ret.insert(key.to_string(), vec![value.to_string()]);
        }
    }
    ret
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