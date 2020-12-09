// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::future::Future;
use std::sync::Arc;

use hyper::{Body, Request, Method};
use path_tree::PathTree;

use crate::server::{Handler, HResult, Params, Query, RpcServiceEnvironment};
use crate::server::{dev_handler, protocol_handler, shell_handler};

pub(crate) fn create_routes(is_sandbox: bool) -> PathTree<Handler> {
    let mut routes = PathTree::<Handler>::new();

    // Shell rpc - implemented
    routes.handle(Method::GET, "/version", shell_handler::node_version);
    routes.handle(Method::GET, "/monitor/bootstrapped", shell_handler::bootstrapped);
    routes.handle(Method::GET, "/monitor/commit_hash", shell_handler::commit_hash);
    routes.handle(Method::GET, "/monitor/active_chains", shell_handler::active_chains);
    routes.handle(Method::GET, "/monitor/protocols", shell_handler::protocols);
    routes.handle(Method::GET, "/monitor/valid_blocks", shell_handler::valid_blocks);
    routes.handle(Method::GET, "/monitor/heads/:chain_id", shell_handler::head_chain);
    routes.handle(Method::GET, "/chains/:chain_id/chain_id", shell_handler::get_chain_id);
    routes.handle(Method::GET, "/chains/:chain_id/blocks", shell_handler::blocks);
    routes.handle(Method::GET, "/chains/:chain_id/blocks/:block_id", shell_handler::chains_block_id);
    routes.handle(Method::GET, "/chains/:chain_id/blocks/:block_id/live_blocks", shell_handler::live_blocks);
    routes.handle(Method::GET, "/chains/:chain_id/blocks/:block_id/header", shell_handler::chains_block_id_header);
    routes.handle(Method::GET, "/chains/:chain_id/blocks/:block_id/header/shell", shell_handler::chains_block_id_header_shell);
    routes.handle(Method::GET, "/chains/:chain_id/mempool/pending_operations", shell_handler::mempool_pending_operations);
    routes.handle(Method::GET, "/chains/:chain_id/mempool/monitor_operations", shell_handler::mempool_monitor_operations);
    routes.handle(Method::POST, "/chains/:chain_id/mempool/request_operations", shell_handler::mempool_request_operations);
    routes.handle(Method::GET, "/chains/:chain_id/blocks/:block_id/protocols", shell_handler::get_block_protocols);
    routes.handle(Method::GET, "/chains/:chain_id/blocks/:block_id/hash", shell_handler::get_block_hash);
    routes.handle(Method::GET, "/chains/:chain_id/blocks/:block_id/operation_hashes", shell_handler::get_block_operation_hashes);
    routes.handle(Method::GET, "/chains/:chain_id/blocks/:block_id/context/raw/bytes", shell_handler::context_raw_bytes);
    routes.handle(Method::GET, "/chains/:chain_id/blocks/:block_id/context/raw/bytes/*any", shell_handler::context_raw_bytes);
    routes.handle(Method::GET, "/chains/:chain_id/blocks/:block_id/metadata", shell_handler::chains_block_id_metadata);
    routes.handle(Method::POST, "/injection/operation", shell_handler::inject_operation);
    routes.handle(Method::GET, "/workers/prevalidators", shell_handler::worker_prevalidators);
    routes.handle(Method::GET, "/config/network/user_activated_upgrades", shell_handler::config_user_activated_upgrades);
    routes.handle(Method::GET, "/config/network/user_activated_protocol_overrides", shell_handler::config_user_activated_protocol_overrides);
    // TODO: TE-174: just for sandbox
    if is_sandbox {
        routes.handle(Method::POST, "/injection/block", shell_handler::inject_block);
    }

    // Shell rpcs - routed through ffi calls
    routes.handle(Method::GET, "/chains/:chain_id/blocks/:block_id/helpers/preapply/operations", shell_handler::preapply_operations);
    routes.handle(Method::GET, "/chains/:chain_id/blocks/:block_id/helpers/preapply/block", shell_handler::preapply_block);

    // Protocol rpcs - implemented
    routes.handle(Method::GET, "/chains/:chain_id/blocks/:block_id/context/constants", protocol_handler::context_constants);
    routes.handle(Method::GET, "/chains/:chain_id/blocks/:block_id/helpers/baking_rights", protocol_handler::baking_rights);
    routes.handle(Method::GET, "/chains/:chain_id/blocks/:block_id/helpers/endorsing_rights", protocol_handler::endorsing_rights);
    routes.handle(Method::GET, "/chains/:chain_id/blocks/:block_id/votes/listings", protocol_handler::votes_listings);

    // Other Protocol rpcs - routed through ffi calls
    // TODO: rework this, this should accept more methods
    routes.handle(Method::GET, "/chains/:chain_id/blocks/:block_id/*any", protocol_handler::call_protocol_rpc);

    // Tezedge dev and support rpcs
    routes.handle(Method::GET, "/dev/chains/main/blocks", dev_handler::dev_blocks);
    routes.handle(Method::GET, "/dev/chains/main/actions/blocks/:block_hash", dev_handler::dev_action_cursor);
    routes.handle(Method::GET, "/dev/chains/main/actions/contracts/:contract_address", dev_handler::dev_action_cursor);
    routes.handle(Method::GET, "/stats/memory", dev_handler::dev_stats_memory);
    routes.handle(Method::GET, "/stats/database_mem", dev_handler::database_memstats);
    //routes.handle(Method::GET, "/stats/storage", dev_handler::dev_stats_storage);

    // DEPRECATED in ocaml but still used by python tests
    routes.handle(Method::GET, "/network/version", shell_handler::node_version);
    
    routes
}

trait Routes<Fut> {
    fn handle(&mut self, method: Method, path: &str, f: Fut);
}

impl<T, F> Routes<T> for PathTree<Handler>
    where
        T: Fn(Request<Body>, Params, Query, RpcServiceEnvironment) -> F + Send + Sync + 'static,
        F: Future<Output=HResult> + Send + 'static
{
    fn handle(&mut self, method: Method, path: &str, f: T) {
        self.insert(path, Arc::new(move |req, params, query, env| {
            Box::new(f(req, params, query, env))
        }));
        self.insert(&format!("/describe{}", path), Arc::new(move |req, params, query, env| {
            Box::new(shell_handler::describe(method.clone(), req, params, query, env))
        }));
    }
}
