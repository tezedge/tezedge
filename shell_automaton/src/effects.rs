// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::actors::actors_effects;
use crate::block_applier::block_applier_effects;
use crate::bootstrap::{bootstrap_effects, BootstrapCheckTimeoutsInitAction};
use crate::current_head::current_head_effects;
use crate::current_head_precheck::current_head_precheck_effects;
use crate::prechecker::prechecker_effects;
use crate::protocol_runner::current_head::protocol_runner_current_head_effects;
use crate::rights::rights_effects;
use crate::service::storage_service::{StorageRequest, StorageRequestPayload};
use crate::service::{Service, StorageService};
use crate::shutdown::shutdown_effects;
use crate::stats::current_head::stats_current_head_effects;
use crate::storage::blocks::genesis::init::commit_result_get::storage_blocks_genesis_init_commit_result_get_effects;
use crate::storage::blocks::genesis::init::commit_result_put::storage_blocks_genesis_init_commit_result_put_effects;
use crate::{Action, ActionWithMeta, Store};

use crate::logger::logger_effects;
use crate::paused_loops::paused_loops_effects;

use crate::protocol_runner::init::context::protocol_runner_init_context_effects;
use crate::protocol_runner::init::context_ipc_server::protocol_runner_init_context_ipc_server_effects;
use crate::protocol_runner::init::protocol_runner_init_effects;
use crate::protocol_runner::init::runtime::protocol_runner_init_runtime_effects;
use crate::protocol_runner::protocol_runner_effects;
use crate::protocol_runner::spawn_server::protocol_runner_spawn_server_effects;

use crate::peer::binary_message::read::peer_binary_message_read_effects;
use crate::peer::binary_message::write::peer_binary_message_write_effects;
use crate::peer::chunk::read::peer_chunk_read_effects;
use crate::peer::chunk::write::peer_chunk_write_effects;
use crate::peer::connection::closed::peer_connection_closed_effects;
use crate::peer::connection::incoming::accept::peer_connection_incoming_accept_effects;
use crate::peer::connection::incoming::peer_connection_incoming_effects;
use crate::peer::connection::outgoing::peer_connection_outgoing_effects;
use crate::peer::disconnection::peer_disconnection_effects;
use crate::peer::handshaking::peer_handshaking_effects;
use crate::peer::message::read::peer_message_read_effects;
use crate::peer::message::write::peer_message_write_effects;
use crate::peer::peer_effects;
use crate::peer::remote_requests::block_header_get::peer_remote_requests_block_header_get_effects;
use crate::peer::remote_requests::block_operations_get::peer_remote_requests_block_operations_get_effects;
use crate::peer::remote_requests::current_branch_get::peer_remote_requests_current_branch_get_effects;
use crate::peer::requests::potential_peers_get::peer_requests_potential_peers_get_effects;

use crate::peers::add::multi::peers_add_multi_effects;
use crate::peers::check::timeouts::{peers_check_timeouts_effects, PeersCheckTimeoutsInitAction};
use crate::peers::dns_lookup::peers_dns_lookup_effects;
use crate::peers::graylist::peers_graylist_effects;
use crate::peers::init::peers_init_effects;

use crate::mempool::mempool_effects;
use crate::mempool::validator::mempool_validator_effects;

use crate::storage::blocks::genesis::check_applied::storage_blocks_genesis_check_applied_effects;
use crate::storage::blocks::genesis::init::additional_data_put::storage_blocks_genesis_init_additional_data_put_effects;
use crate::storage::blocks::genesis::init::header_put::storage_blocks_genesis_init_header_put_effects;
use crate::storage::blocks::genesis::init::storage_blocks_genesis_init_effects;
use crate::storage::request::storage_request_effects;
use crate::storage::state_snapshot::create::{
    storage_state_snapshot_create_effects, StorageStateSnapshotCreateInitAction,
};

use crate::storage::{
    kv_block_additional_data::effects as kv_block_additional_data_effects,
    kv_block_header::effects as kv_block_header_effects,
    kv_block_meta::effects as kv_block_meta_effects, kv_constants::effects as kv_constants_effects,
    kv_cycle_eras::effects as kv_cycle_eras_effects,
    kv_cycle_meta::effects as kv_cycle_meta_effects,
    kv_operations::effects as kv_operations_effects,
};

use crate::rpc::rpc_effects;

fn last_action_effects<S: Service>(store: &mut Store<S>, action: &ActionWithMeta) {
    if let Some(stats) = store.service.statistics() {
        stats.action_new(action)
    }

    if !store.state.get().config.record_actions {
        return;
    }

    let _ = store.service.storage().request_send(StorageRequest::new(
        None,
        StorageRequestPayload::ActionPut(Box::new(action.clone())),
    ));
}

fn applied_actions_count_effects<S: Service>(store: &mut Store<S>, action: &ActionWithMeta) {
    if !matches!(&action.action, Action::StorageStateSnapshotCreateInit(_)) {
        store.dispatch(StorageStateSnapshotCreateInitAction {});
    }
}

/// All the actions which trigger checking for timeouts are called here.
pub fn check_timeouts<S: Service>(store: &mut Store<S>) {
    store.dispatch(PeersCheckTimeoutsInitAction {});
    store.dispatch(BootstrapCheckTimeoutsInitAction {});
}

pub fn effects<S: Service>(store: &mut Store<S>, action: &ActionWithMeta) {
    // these four effects must be first and in this order!
    // if action.action.as_ref().starts_with("Rights") {
    //     slog::debug!(store.state().log, "Rights action"; "action" => format!("{:#?}", action.action));
    // }

    logger_effects(store, action);
    last_action_effects(store, action);
    applied_actions_count_effects(store, action);

    paused_loops_effects(store, action);

    stats_current_head_effects(store, action);

    protocol_runner_effects(store, action);

    protocol_runner_spawn_server_effects(store, action);

    protocol_runner_current_head_effects(store, action);

    protocol_runner_init_effects(store, action);
    protocol_runner_init_runtime_effects(store, action);
    protocol_runner_init_context_effects(store, action);
    protocol_runner_init_context_ipc_server_effects(store, action);

    current_head_effects(store, action);

    block_applier_effects(store, action);

    peer_effects(store, action);

    peer_connection_outgoing_effects(store, action);
    peer_connection_incoming_accept_effects(store, action);
    peer_connection_incoming_effects(store, action);
    peer_connection_closed_effects(store, action);
    peer_disconnection_effects(store, action);

    peer_message_read_effects(store, action);
    peer_message_write_effects(store, action);
    peer_binary_message_write_effects(store, action);
    peer_binary_message_read_effects(store, action);
    peer_chunk_write_effects(store, action);
    peer_chunk_read_effects(store, action);

    peer_handshaking_effects(store, action);

    peer_requests_potential_peers_get_effects(store, action);

    peer_remote_requests_block_header_get_effects(store, action);
    peer_remote_requests_block_operations_get_effects(store, action);
    peer_remote_requests_current_branch_get_effects(store, action);

    peers_init_effects(store, action);
    peers_dns_lookup_effects(store, action);
    peers_add_multi_effects(store, action);
    peers_check_timeouts_effects(store, action);
    peers_graylist_effects(store, action);

    bootstrap_effects(store, action);
    mempool_validator_effects(store, action);
    mempool_effects(store, action);

    storage_request_effects(store, action);

    storage_blocks_genesis_check_applied_effects(store, action);

    storage_blocks_genesis_init_effects(store, action);
    storage_blocks_genesis_init_header_put_effects(store, action);
    storage_blocks_genesis_init_additional_data_put_effects(store, action);
    storage_blocks_genesis_init_commit_result_get_effects(store, action);
    storage_blocks_genesis_init_commit_result_put_effects(store, action);

    storage_state_snapshot_create_effects(store, action);

    actors_effects(store, action);
    rpc_effects(store, action);

    rights_effects(store, action);

    current_head_precheck_effects(store, action);

    prechecker_effects(store, action);

    kv_block_meta_effects(store, action);
    kv_block_header_effects(store, action);
    kv_block_additional_data_effects(store, action);
    kv_constants_effects(store, action);
    kv_cycle_eras_effects(store, action);
    kv_cycle_meta_effects(store, action);
    kv_operations_effects(store, action);

    shutdown_effects(store, action);
}
