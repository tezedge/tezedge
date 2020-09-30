// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::future::Future;
use std::sync::Arc;

use hyper::{Body, Request};
use path_tree::PathTree;

use crate::server::{Handler, HResult, Params, Query, RpcServiceEnvironment};
use crate::server::{dev_handler, handler};

pub(crate) fn create_routes(is_sandbox: bool) -> PathTree<Handler> {
    let mut routes = PathTree::<Handler>::new();

    // Shell rpc - implemented
    routes.handle("/version", handler::node_version);
    routes.handle("/database_memstats", handler::database_memstats);
    routes.handle("/monitor/bootstrapped", handler::bootstrapped);
    routes.handle("/monitor/commit_hash", handler::commit_hash);
    routes.handle("/monitor/active_chains", handler::active_chains);
    routes.handle("/monitor/protocols", handler::protocols);
    routes.handle("/monitor/valid_blocks", handler::valid_blocks);
    routes.handle("/monitor/heads/:chain_id", handler::head_chain);
    routes.handle("/chains/:chain_id/chain_id", handler::get_chain_id);
    routes.handle("/chains/:chain_id/blocks/:block_id", handler::chains_block_id);
    routes.handle("/chains/:chain_id/blocks/:block_id/live_blocks", handler::live_blocks);
    routes.handle("/chains/:chain_id/blocks/:block_id/header", handler::chains_block_id_header);
    routes.handle("/chains/:chain_id/blocks/:block_id/header/shell", handler::chains_block_id_header_shell);
    routes.handle("/chains/:chain_id/mempool/pending_operations", handler::mempool_pending_operations);
    routes.handle("/chains/:chain_id/blocks/:block_id/protocols", handler::get_block_protocols);
    routes.handle("/chains/:chain_id/blocks/:block_id/hash", handler::get_block_hash);
    routes.handle("/chains/:chain_id/blocks/:block_id/operation_hashes", handler::get_block_operation_hashes);
    routes.handle("/injection/operation", handler::inject_operation);

    // TODO: TE-226 - implement correctly or just remove, it will be part of protocol router
    // there should be just two endpoints: context/raw/json (from protocol), context/raw/bytes (shell rpc)
    // both should return just value (bytes or json) for key, which is part of uri, like: `context/raw/json/rolls/owner/current` -> rolls/owner/current is key to context
    routes.handle("/chains/:chain_id/blocks/:block_id/context/raw/bytes/cycle", handler::context_cycle);
    routes.handle("/chains/:chain_id/blocks/:block_id/context/raw/bytes/rolls/owner/current", handler::rolls_owner_current);

    // TODO: TE-174: just for sandbox
    if is_sandbox {
        routes.handle("/injection/block", handler::inject_block);
    }

    // Shell rpcs - routed through ffi calls
    routes.handle("/chains/:chain_id/blocks/:block_id/helpers/preapply/operations", handler::preapply_operations);
    routes.handle("/chains/:chain_id/blocks/:block_id/helpers/preapply/block", handler::preapply_block);

    // Protocol rpcs - implemented
    routes.handle("/chains/:chain_id/blocks/:block_id/context/constants", handler::context_constants);
    routes.handle("/chains/:chain_id/blocks/:block_id/context/raw/json/cycle/:cycle_id", handler::cycle);
    routes.handle("/chains/:chain_id/blocks/:block_id/context/contracts/:pkh", handler::context_contract);
    routes.handle("/chains/:chain_id/blocks/:block_id/context/contracts/:pkh/counter", handler::get_contract_counter);
    routes.handle("/chains/:chain_id/blocks/:block_id/context/contracts/:pkh/manager_key", handler::get_contract_manager_key);
    routes.handle("/chains/:chain_id/blocks/:block_id/helpers/baking_rights", handler::baking_rights);
    routes.handle("/chains/:chain_id/blocks/:block_id/helpers/endorsing_rights", handler::endorsing_rights);
    routes.handle("/chains/:chain_id/blocks/:block_id/votes/listings", handler::votes_listings);

    // Protocol rpcs - routed through ffi calls
    routes.handle("/chains/:chain_id/blocks/:block_id/helpers/scripts/run_operation", handler::run_operation);
    routes.handle("/chains/:chain_id/blocks/:block_id/helpers/forge/operations", handler::forge_operations);
    routes.handle("/chains/:chain_id/blocks/:block_id/helpers/current_level", handler::current_level);
    routes.handle("/chains/:chain_id/blocks/:block_id/minimal_valid_time", handler::minimal_valid_time);

    // Tezedge dev and support rpcs
    routes.handle("/dev/chains/main/blocks", dev_handler::dev_blocks);
    routes.handle("/dev/chains/main/actions/blocks/:block_hash", dev_handler::dev_action_cursor);
    routes.handle("/dev/chains/main/actions/contracts/:contract_address", dev_handler::dev_action_cursor);
    routes.handle("/stats/memory", dev_handler::dev_stats_memory);
    //routes.handle("/stats/storage", dev_handler::dev_stats_storage);

    routes
}

trait Routes<Fut> {
    fn handle(&mut self, path: &str, f: Fut);
}

impl<T, F> Routes<T> for PathTree<Handler>
    where
        T: Fn(Request<Body>, Params, Query, RpcServiceEnvironment) -> F + Send + Sync + 'static,
        F: Future<Output=HResult> + Send + 'static
{
    fn handle(&mut self, path: &str, f: T) {
        self.insert(path, Arc::new(move |req, params, query, env| {
            Box::new(f(req, params, query, env))
        }));
    }
}
