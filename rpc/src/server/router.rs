// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::future::Future;
use std::sync::Arc;

use hyper::{Body, Request};
use path_tree::PathTree;

use crate::server::{Handler, HResult, Params, Query, RpcServiceEnvironment};
use crate::server::{dev_handler, protocol_handler, shell_handler};

pub(crate) fn create_routes(is_sandbox: bool) -> PathTree<Handler> {
    let mut routes = PathTree::<Handler>::new();

    // Shell rpc - implemented
    routes.handle("/version", shell_handler::node_version);
    routes.handle("/monitor/bootstrapped", shell_handler::bootstrapped);
    routes.handle("/monitor/commit_hash", shell_handler::commit_hash);
    routes.handle("/monitor/active_chains", shell_handler::active_chains);
    routes.handle("/monitor/protocols", shell_handler::protocols);
    routes.handle("/monitor/valid_blocks", shell_handler::valid_blocks);
    routes.handle("/monitor/heads/:chain_id", shell_handler::head_chain);
    routes.handle("/chains/:chain_id/chain_id", shell_handler::get_chain_id);
    routes.handle("/chains/:chain_id/blocks/:block_id", shell_handler::chains_block_id);
    routes.handle("/chains/:chain_id/blocks/:block_id/live_blocks", shell_handler::live_blocks);
    routes.handle("/chains/:chain_id/blocks/:block_id/header", shell_handler::chains_block_id_header);
    routes.handle("/chains/:chain_id/blocks/:block_id/header/shell", shell_handler::chains_block_id_header_shell);
    routes.handle("/chains/:chain_id/mempool/pending_operations", shell_handler::mempool_pending_operations);
    routes.handle("/chains/:chain_id/blocks/:block_id/protocols", shell_handler::get_block_protocols);
    routes.handle("/chains/:chain_id/blocks/:block_id/hash", shell_handler::get_block_hash);
    routes.handle("/chains/:chain_id/blocks/:block_id/operation_hashes", shell_handler::get_block_operation_hashes);
    routes.handle("/chains/:chain_id/blocks/:block_id/context/raw/bytes", shell_handler::context_raw_bytes);
    routes.handle("/chains/:chain_id/blocks/:block_id/context/raw/bytes/*any", shell_handler::context_raw_bytes);
    routes.handle("/injection/operation", shell_handler::inject_operation);
    // TODO: TE-174: just for sandbox
    if is_sandbox {
        routes.handle("/injection/block", shell_handler::inject_block);
    }

    // Shell rpcs - routed through ffi calls
    routes.handle("/chains/:chain_id/blocks/:block_id/helpers/preapply/operations", shell_handler::preapply_operations);
    routes.handle("/chains/:chain_id/blocks/:block_id/helpers/preapply/block", shell_handler::preapply_block);

    // Protocol rpcs - implemented
    routes.handle("/chains/:chain_id/blocks/:block_id/context/constants", protocol_handler::context_constants);
    routes.handle("/chains/:chain_id/blocks/:block_id/context/raw/json/cycle/:cycle_id", protocol_handler::cycle);
    routes.handle("/chains/:chain_id/blocks/:block_id/context/contracts/:pkh", protocol_handler::context_contract);
    routes.handle("/chains/:chain_id/blocks/:block_id/context/contracts/:pkh/counter", protocol_handler::get_contract_counter);
    routes.handle("/chains/:chain_id/blocks/:block_id/context/contracts/:pkh/manager_key", protocol_handler::get_contract_manager_key);
    routes.handle("/chains/:chain_id/blocks/:block_id/helpers/baking_rights", protocol_handler::baking_rights);
    routes.handle("/chains/:chain_id/blocks/:block_id/helpers/endorsing_rights", protocol_handler::endorsing_rights);
    routes.handle("/chains/:chain_id/blocks/:block_id/votes/listings", protocol_handler::votes_listings);

    // Protocol rpcs - routed through ffi calls
    routes.handle("/chains/:chain_id/blocks/:block_id/helpers/scripts/run_operation", protocol_handler::run_operation);
    routes.handle("/chains/:chain_id/blocks/:block_id/helpers/forge/operations", protocol_handler::forge_operations);
    routes.handle("/chains/:chain_id/blocks/:block_id/helpers/current_level", protocol_handler::current_level);
    routes.handle("/chains/:chain_id/blocks/:block_id/minimal_valid_time", protocol_handler::minimal_valid_time);

    // Tezedge dev and support rpcs
    routes.handle("/dev/chains/main/blocks", dev_handler::dev_blocks);
    routes.handle("/dev/chains/main/actions/blocks/:block_hash", dev_handler::dev_action_cursor);
    routes.handle("/dev/chains/main/actions/contracts/:contract_address", dev_handler::dev_action_cursor);
    routes.handle("/stats/memory", dev_handler::dev_stats_memory);
    routes.handle("/stats/database_mem", dev_handler::database_memstats);
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
