// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use getset::{CopyGetters, Getters};
use hyper::server::conn::AddrStream;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response};
use slog::{error, Logger};
use tezos_protocol_ipc_client::ProtocolRunnerApi;
use tokio::runtime::Handle;

use crypto::hash::ChainId;
use shell_automaton::service::rpc_service::RpcShellAutomatonSender;
use shell_integration::{StreamCounter, StreamWakers};
use storage::{BlockHeaderWithHash, PersistentStorage};
use tezos_api::environment::TezosEnvironmentConfiguration;
use tezos_context_ipc_client::TezedgeContextClient;
use tezos_messages::p2p::encoding::version::NetworkVersion;
use url::Url;

use crate::{error_with_message, not_found, options};

mod dev_handler;
mod openapi_handler;
mod protocol_handler;
mod router;
pub(crate) mod rpc_server;
mod shell_handler;

/// Thread safe reference to a shared RPC state
pub type RpcCollectedStateRef = Arc<RwLock<RpcCollectedState>>;

/// Thread safe reference to a shared RPC environment configuration
pub type RpcServiceEnvironmentRef = Arc<RpcServiceEnvironment>;

/// Represents various collected information about
/// internal state of the node.
#[derive(CopyGetters, Getters)]
pub struct RpcCollectedState {
    #[get = "pub(crate)"]
    current_head: Arc<BlockHeaderWithHash>,
    #[get = "pub(crate)"]
    best_remote_level: Option<i32>,

    // Wakers for open streams (monitors) that access the mempool state
    streams: StreamWakers,
}

impl StreamCounter for RpcCollectedState {
    fn get_streams(&self) -> &StreamWakers {
        &self.streams
    }

    fn get_mutable_streams(&mut self) -> &mut StreamWakers {
        &mut self.streams
    }
}

/// Server environment parameters
#[derive(Getters)]
pub struct RpcServiceEnvironment {
    #[get = "pub(crate)"]
    persistent_storage: PersistentStorage,
    #[get = "pub(crate)"]
    state: RpcCollectedStateRef,
    #[get = "pub(crate)"]
    shell_automaton_sender: RpcShellAutomatonSender,
    #[get = "pub(crate)"]
    tezos_environment: TezosEnvironmentConfiguration,
    #[get = "pub(crate)"]
    network_version: Arc<NetworkVersion>,
    #[get = "pub(crate)"]
    log: Logger,
    #[get = "pub(crate)"]
    tokio_executor: Arc<Handle>,

    #[get = "pub(crate)"]
    main_chain_id: ChainId,

    #[get = "pub(crate)"]
    tezedge_context: TezedgeContextClient,
    #[get = "pub(crate)"]
    tezos_protocol_api: Arc<ProtocolRunnerApi>,
    #[get = "pub(crate)"]
    context_stats_db_path: Option<PathBuf>,
    pub tezedge_is_enabled: bool,
}

impl RpcServiceEnvironment {
    pub fn new(
        tokio_executor: Arc<Handle>,
        shell_automaton_sender: RpcShellAutomatonSender,
        tezos_environment: TezosEnvironmentConfiguration,
        network_version: Arc<NetworkVersion>,
        persistent_storage: &PersistentStorage,
        tezos_protocol_api: Arc<ProtocolRunnerApi>,
        main_chain_id: ChainId,
        state: RpcCollectedStateRef,
        context_stats_db_path: Option<PathBuf>,
        tezedge_is_enabled: bool,
        log: Logger,
    ) -> Self {
        let tezedge_context = TezedgeContextClient::new(Arc::clone(&tezos_protocol_api));
        Self {
            tokio_executor,
            shell_automaton_sender,
            tezos_environment,
            network_version,
            persistent_storage: persistent_storage.clone(),
            main_chain_id,
            state,
            log,
            tezedge_context,
            tezos_protocol_api,
            context_stats_db_path,
            tezedge_is_enabled,
        }
    }
}

pub type Params = Vec<(String, String)>;

pub type Query = HashMap<String, Vec<String>>;

pub type HResult = Result<Response<Body>, Box<dyn std::error::Error + Sync + Send>>;

pub type Handler = Arc<
    dyn Fn(
            Request<Body>,
            Params,
            Query,
            Arc<RpcServiceEnvironment>,
        ) -> Box<dyn Future<Output = HResult> + Send>
        + Send
        + Sync,
>;

pub struct MethodHandler {
    allowed_methods: Arc<HashSet<Method>>,
    handler: Handler,
}

impl MethodHandler {
    pub fn new(allowed_methods: Arc<HashSet<Method>>, handler: Handler) -> Self {
        Self {
            allowed_methods,
            handler,
        }
    }
}

/// Spawn new HTTP server on given address interacting with specific actor system
pub fn spawn_server(
    bind_address: &SocketAddr,
    env: Arc<RpcServiceEnvironment>,
) -> impl Future<Output = Result<(), hyper::Error>> {
    let routes = Arc::new(router::create_routes(env.tezedge_is_enabled));

    hyper::Server::bind(bind_address)
        .serve(make_service_fn(move |socket: &AddrStream| {
            let remote_addr = socket.remote_addr();
            let env = env.clone();
            let routes = routes.clone();

            async move {
                Ok::<_, hyper::Error>(service_fn(move |req: Request<Body>| {
                    let env = env.clone();
                    let routes = routes.clone();
                    async move {
                        let log = env.log().clone();
                        let req_method = req.method().clone();
                        let original_path = req.uri().path();
                        let normalized_path = normalize_path(req.uri().path()).unwrap_or_else(|| original_path.to_owned());

                        slog::debug!(&log, "Rpc request";
                            "remote_addr" => remote_addr,
                            "method" => req_method.to_string(),
                            "original_path" => &original_path,
                            "normalized_path" => &normalized_path,
                            "body" => slog::FnValue(|_| {
                                format!("{:?}", req.body())
                            }));

                        let result = if let Some((method_and_handler, params)) = routes.find(normalized_path.trim_end_matches('/')) {
                            let MethodHandler {
                                allowed_methods,
                                handler,
                            } = method_and_handler;

                            let request_method = req.method();

                            match *request_method {
                                Method::OPTIONS => {
                                    // lets globaly handle options
                                    options()
                                }
                                _ => {
                                    if allowed_methods.contains(request_method) {
                                        let params: Params = params.into_iter().map(|(param, value)| (param.to_string(), value.to_string())).collect();
                                        let query: Query = req.uri().query().map(parse_query_string).unwrap_or_else(HashMap::new);

                                        let handler = handler.clone();
                                        let fut = handler(req, params, query, env);
                                        match Pin::from(fut).await {
                                            Ok(response) => Ok(response),
                                            Err(e) => {
                                                error!(log, "Failed to execute RPC function - unhandled error"; "reason" => format!("{:?}", &e));
                                                error_with_message(format!("{:?}", e))
                                            }
                                        }
                                    } else {
                                        let error_message = format!("Failed to execute RPC function - Method {} not registered for this RPC function", request_method);
                                        error!(log, "{}", error_message);
                                        error_with_message(format!("{:?}", error_message))
                                    }
                                }
                            }
                        } else {
                            not_found()
                        };

                        slog::debug!(&log, "Rpc response";
                            "remote_addr" => remote_addr,
                            "method" => req_method.to_string(),
                            "normalized_path" => &normalized_path,
                            "status" => match &result {
                                Ok(v) => v.status().to_string(),
                                Err(_) => "500".to_owned(),
                            },
                            "body" => slog::FnValue(|_| {
                                match &result {
                                    Ok(resp) => format!("{:?}", resp.body()),
                                    Err(err) => format!("Err: {}", err),
                                }
                            }));

                        result
                    }
                }))
            }
        }))
}

/// Normalizes the request path
fn normalize_path(original_path: &str) -> Option<String> {
    let base = Url::parse("http://tezedge.com").ok()?;
    let parsed = Url::options()
        .base_url(Some(&base))
        .parse(original_path)
        .ok()?;
    let non_empty_segments: Vec<_> = parsed.path_segments()?.filter(|s| !s.is_empty()).collect();
    let reconstructed = non_empty_segments.join("/");

    Some(format!("/{}", reconstructed))
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
        self.get_str(key)
            .and_then(|value| value.parse::<u64>().ok())
    }

    fn get_usize(&self, key: &str) -> Option<usize> {
        self.get_str(key)
            .and_then(|value| value.parse::<usize>().ok())
    }

    fn contains_key(&self, key: &str) -> bool {
        self.get_str(key).is_some()
    }
}

impl HasSingleValue for Params {
    fn get_str(&self, key: &str) -> Option<&str> {
        self.iter()
            .find_map(|(k, v)| if k == key { Some(v.as_str()) } else { None })
    }
}

impl HasSingleValue for Query {
    fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key)
            .map(|values| values.iter().next().map(String::as_str))
            .flatten()
    }
}
