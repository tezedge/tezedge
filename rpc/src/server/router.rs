// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashSet;
use std::future::Future;
use std::sync::Arc;

use hyper::{Body, Method, Request};
use path_tree::PathTree;

use crate::server::{dev_handler, openapi_handler, protocol_handler, shell_handler};
use crate::server::{HResult, MethodHandler, Params, Query, RpcServiceEnvironment};

macro_rules! hash_set {
    ( $( $x:expr ),* ) => {
        {
            let mut temp_set = HashSet::new();
            $(
                temp_set.insert($x);
            )*
            temp_set
        }
    };
}

pub(crate) fn create_routes(tezedge_is_enabled: bool) -> PathTree<MethodHandler> {
    let mut routes = PathTree::<MethodHandler>::new();

    // Shell rpc - implemented
    routes.handle(
        hash_set![Method::GET],
        "/version",
        shell_handler::node_version,
    );
    routes.handle(
        hash_set![Method::GET],
        "/monitor/bootstrapped",
        shell_handler::bootstrapped,
    );
    routes.handle(
        hash_set![Method::GET],
        "/monitor/commit_hash",
        shell_handler::commit_hash,
    );
    routes.handle(
        hash_set![Method::GET],
        "/monitor/active_chains",
        shell_handler::active_chains,
    );
    routes.handle(
        hash_set![Method::GET],
        "/monitor/protocols",
        shell_handler::protocols,
    );
    routes.handle(
        hash_set![Method::GET],
        "/monitor/valid_blocks",
        shell_handler::valid_blocks,
    );
    routes.handle(
        hash_set![Method::GET],
        "/monitor/heads/:chain_id",
        shell_handler::head_chain,
    );
    routes.handle(
        hash_set![Method::GET],
        "/chains/:chain_id/chain_id",
        shell_handler::get_chain_id,
    );
    routes.handle(
        hash_set![Method::GET],
        "/chains/:chain_id/blocks",
        shell_handler::blocks,
    );
    routes.handle(
        hash_set![Method::GET],
        "/chains/:chain_id/blocks/:block_id",
        shell_handler::chains_block_id,
    );
    routes.handle(
        hash_set![Method::GET],
        "/chains/:chain_id/blocks/:block_id/live_blocks",
        shell_handler::live_blocks,
    );
    routes.handle(
        hash_set![Method::GET],
        "/chains/:chain_id/blocks/:block_id/header",
        shell_handler::chains_block_id_header,
    );
    routes.handle(
        hash_set![Method::GET],
        "/chains/:chain_id/blocks/:block_id/header/shell",
        shell_handler::chains_block_id_header_shell,
    );
    routes.handle(
        hash_set![Method::GET],
        "/chains/:chain_id/mempool/pending_operations",
        shell_handler::mempool_pending_operations,
    );
    routes.handle(
        hash_set![Method::GET],
        "/chains/:chain_id/mempool/monitor_operations",
        shell_handler::mempool_monitor_operations,
    );
    routes.handle(
        hash_set![Method::POST],
        "/chains/:chain_id/mempool/request_operations",
        shell_handler::mempool_request_operations,
    );
    routes.handle(
        hash_set![Method::GET],
        "/chains/:chain_id/blocks/:block_id/protocols",
        shell_handler::get_block_protocols,
    );
    routes.handle(
        hash_set![Method::GET],
        "/chains/:chain_id/blocks/:block_id/hash",
        shell_handler::get_block_hash,
    );
    routes.handle(
        hash_set![Method::GET],
        "/chains/:chain_id/blocks/:block_id/metadata_hash",
        shell_handler::get_metadata_hash,
    );
    routes.handle(
        hash_set![Method::GET],
        "/chains/:chain_id/blocks/:block_id/operations_metadata_hash",
        shell_handler::get_operations_metadata_hash,
    );
    routes.handle(
        hash_set![Method::GET],
        "/chains/:chain_id/blocks/:block_id/operation_metadata_hashes",
        shell_handler::get_operations_metadata_hash_operation_metadata_hashes,
    );
    routes.handle(
        hash_set![Method::GET],
        "/chains/:chain_id/blocks/:block_id/operation_metadata_hashes/:validation_pass_index",
        shell_handler::get_operations_metadata_hash_operation_metadata_hashes_by_validation_pass,
    );
    routes.handle(
        hash_set![Method::GET],
        "/chains/:chain_id/blocks/:block_id/operation_metadata_hashes/:validation_pass_index/:operation_index",
        shell_handler::get_operations_metadata_hash_operation_metadata_hashes_by_validation_pass_by_operation_index,
    );
    routes.handle(
        hash_set![Method::GET],
        "/chains/:chain_id/blocks/:block_id/operation_hashes",
        shell_handler::get_block_operation_hashes,
    );
    routes.handle(
        hash_set![Method::GET],
        "/chains/:chain_id/blocks/:block_id/operations",
        shell_handler::get_block_operations,
    );
    routes.handle(
        hash_set![Method::GET],
        "/chains/:chain_id/blocks/:block_id/operations/:validation_pass_index",
        shell_handler::get_block_operations_validation_pass,
    );
    routes.handle(
        hash_set![Method::GET],
        "/chains/:chain_id/blocks/:block_id/operations/:validation_pass_index/:operation_index",
        shell_handler::get_block_operation,
    );

    // TODO - TE-261: we are routing these to OCaml for now, even when the TezEdge
    // context is available. Once these handlers have been tested better and revised, enable again.
    // TODO - TE-261 - remove this function, it is just to make clippy happy
    fn enable_tezedge_rpcs_with_context(tezedge_is_enabled: bool, really_allow: bool) -> bool {
        tezedge_is_enabled && really_allow
    }

    if enable_tezedge_rpcs_with_context(tezedge_is_enabled, false) {
        routes.handle(
            hash_set![Method::GET],
            "/chains/:chain_id/blocks/:block_id/context/raw/bytes",
            shell_handler::context_raw_bytes,
        );
        routes.handle(
            hash_set![Method::GET],
            "/chains/:chain_id/blocks/:block_id/context/raw/bytes/*any",
            shell_handler::context_raw_bytes,
        );
    }
    routes.handle(
        hash_set![Method::GET],
        "/chains/:chain_id/blocks/:block_id/metadata",
        shell_handler::chains_block_id_metadata,
    );
    routes.handle(
        hash_set![Method::GET],
        "/workers/prevalidators",
        shell_handler::worker_prevalidators,
    );
    routes.handle(
        hash_set![Method::GET],
        "/config/network/user_activated_upgrades",
        shell_handler::config_user_activated_upgrades,
    );
    routes.handle(
        hash_set![Method::GET],
        "/config/network/user_activated_protocol_overrides",
        shell_handler::config_user_activated_protocol_overrides,
    );
    routes.handle(
        hash_set![Method::POST],
        "/injection/operation",
        shell_handler::inject_operation,
    );
    routes.handle(
        hash_set![Method::POST],
        "/injection/block",
        shell_handler::inject_block,
    );

    // Shell rpcs - routed through ffi calls
    routes.handle(
        hash_set![Method::POST],
        "/chains/:chain_id/blocks/:block_id/helpers/preapply/operations",
        shell_handler::preapply_operations,
    );
    routes.handle(
        hash_set![Method::POST],
        "/chains/:chain_id/blocks/:block_id/helpers/preapply/block",
        shell_handler::preapply_block,
    );

    // Protocol rpcs - implemented
    routes.handle(
        hash_set![Method::GET],
        "/chains/:chain_id/blocks/:block_id/helpers/baking_rights",
        protocol_handler::baking_rights,
    );
    routes.handle(
        hash_set![Method::GET],
        "/chains/:chain_id/blocks/:block_id/helpers/endorsing_rights",
        protocol_handler::endorsing_rights,
    );

    // TODO - TE-261: we are routing these to OCaml for now, even when the TezEdge
    // context is available. Once these handlers have been tested better and revised, enable again.
    if enable_tezedge_rpcs_with_context(tezedge_is_enabled, false) {
        // These only work if the TezEdge context is available
        routes.handle(
            hash_set![Method::GET],
            "/chains/:chain_id/blocks/:block_id/context/constants",
            protocol_handler::context_constants,
        );
        routes.handle(
            hash_set![Method::GET],
            "/chains/:chain_id/blocks/:block_id/votes/listings",
            protocol_handler::votes_listings,
        );
    }

    // Other Protocol rpcs - routed through ffi calls
    routes.handle(
        hash_set![Method::GET, Method::POST, Method::OPTIONS, Method::PUT],
        "/chains/:chain_id/blocks/:block_id/*any",
        protocol_handler::call_protocol_rpc,
    );

    // Tezedge dev and support rpcs
    routes.handle(
        hash_set![Method::GET],
        "/dev/chains/main/blocks",
        dev_handler::dev_blocks,
    );
    routes.handle(
        hash_set![Method::GET],
        "/dev/chains/main/actions/blocks/:block_hash",
        dev_handler::dev_action_cursor,
    );
    routes.handle(
        hash_set![Method::GET],
        "/dev/chains/main/actions/blocks/:block_hash/details",
        dev_handler::block_action_details,
    );
    routes.handle(
        hash_set![Method::GET],
        "/dev/chains/main/actions/contracts/:contract_address",
        dev_handler::dev_action_cursor,
    );
    routes.handle(
        hash_set![Method::GET],
        "/dev/version",
        dev_handler::dev_version,
    );
    routes.handle(
        hash_set![Method::GET],
        "/dev/chains/:chain_id/blocks/:block_id/cycle_eras",
        dev_handler::cycle_eras,
    );
    routes.handle(
        hash_set![Method::GET],
        "/dev/shell/automaton/state",
        dev_handler::dev_shell_automaton_state_get,
    );
    routes.handle(
        hash_set![Method::GET],
        "/dev/shell/automaton/actions",
        dev_handler::dev_shell_automaton_actions_get,
    );
    routes.handle(
        hash_set![Method::GET],
        "/dev/shell/automaton/actions_graph",
        dev_handler::dev_shell_automaton_actions_graph_get,
    );
    routes.handle(
        hash_set![Method::GET],
        "/stats/memory",
        dev_handler::dev_stats_memory,
    );
    routes.handle(
        hash_set![Method::GET],
        "/stats/memory/protocol_runners",
        dev_handler::dev_stats_memory_protocol_runners,
    );
    routes.handle(
        hash_set![Method::GET],
        "/stats/context",
        dev_handler::context_stats,
    );
    routes.handle(
        hash_set![Method::GET],
        "/stats/:chain_id/blocks/:block_id",
        dev_handler::block_actions,
    );

    // DEPRECATED in ocaml but still used by python tests
    routes.handle(
        hash_set![Method::GET],
        "/network/version",
        shell_handler::node_version,
    );

    routes.handle(
        hash_set![Method::GET],
        "/openapi/tezedge-openapi.json",
        openapi_handler::get_spec_file,
    );

    routes
}

trait Routes<Fut> {
    fn handle(&mut self, method: HashSet<Method>, path: &str, f: Fut);
}

impl<T, F> Routes<T> for PathTree<MethodHandler>
where
    T: Fn(Request<Body>, Params, Query, Arc<RpcServiceEnvironment>) -> F + Send + Sync + 'static,
    F: Future<Output = HResult> + Send + 'static,
{
    fn handle(&mut self, allowed_methods: HashSet<Method>, path: &str, f: T) {
        let allowed_methods = Arc::new(allowed_methods);
        self.insert(
            path,
            MethodHandler::new(
                allowed_methods.clone(),
                Arc::new(move |req, params, query, env| Box::new(f(req, params, query, env))),
            ),
        );
        self.insert(
            &format!("/describe{}", path),
            MethodHandler::new(
                Arc::new(hash_set![Method::GET]),
                Arc::new(move |req, params, query, env| {
                    Box::new(shell_handler::describe(
                        allowed_methods.clone(),
                        req,
                        params,
                        query,
                        env,
                    ))
                }),
            ),
        );
    }
}
