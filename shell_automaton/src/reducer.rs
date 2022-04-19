// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use redux_rs::chain_reducers;

use crate::action::{Action, ActionWithMeta};
use crate::State;

use crate::block_applier::block_applier_reducer;
use crate::bootstrap::bootstrap_reducer;
use crate::current_head::current_head_reducer;
use crate::current_head_precheck::current_head_precheck_reducer;
use crate::paused_loops::paused_loops_reducer;

use crate::peer::binary_message::read::peer_binary_message_read_reducer;
use crate::peer::binary_message::write::peer_binary_message_write_reducer;
use crate::peer::chunk::read::peer_chunk_read_reducer;
use crate::peer::chunk::write::peer_chunk_write_reducer;
use crate::peer::connection::incoming::accept::peer_connection_incoming_accept_reducer;
use crate::peer::connection::incoming::peer_connection_incoming_reducer;
use crate::peer::connection::outgoing::peer_connection_outgoing_reducer;
use crate::peer::disconnection::peer_disconnection_reducer;
use crate::peer::handshaking::peer_handshaking_reducer;
use crate::peer::message::read::peer_message_read_reducer;
use crate::peer::message::write::peer_message_write_reducer;
use crate::peer::peer_reducer;
use crate::peer::remote_requests::block_header_get::peer_remote_requests_block_header_get_reducer;
use crate::peer::remote_requests::block_operations_get::peer_remote_requests_block_operations_get_reducer;
use crate::peer::remote_requests::current_branch_get::peer_remote_requests_current_branch_get_reducer;
use crate::peer::requests::potential_peers_get::peer_requests_potential_peers_get_reducer;

use crate::peers::add::multi::peers_add_multi_reducer;
use crate::peers::add::peers_add_reducer;
use crate::peers::check::timeouts::peers_check_timeouts_reducer;
use crate::peers::dns_lookup::peers_dns_lookup_reducer;
use crate::peers::graylist::peers_graylist_reducer;
use crate::peers::remove::peers_remove_reducer;

use crate::mempool::mempool_reducer;
use crate::mempool::validator::mempool_validator_reducer;

use crate::prechecker::prechecker_reducer;
use crate::rights::rights_reducer;

use crate::protocol_runner::current_head::protocol_runner_current_head_reducer;
use crate::protocol_runner::init::context::protocol_runner_init_context_reducer;
use crate::protocol_runner::init::context_ipc_server::protocol_runner_init_context_ipc_server_reducer;
use crate::protocol_runner::init::protocol_runner_init_reducer;
use crate::protocol_runner::init::runtime::protocol_runner_init_runtime_reducer;
use crate::protocol_runner::protocol_runner_reducer;
use crate::protocol_runner::spawn_server::protocol_runner_spawn_server_reducer;

use crate::rpc::rpc_reducer;
use crate::shutdown::shutdown_reducer;
use crate::stats::current_head::stats_current_head_reducer;
use crate::storage::blocks::genesis::check_applied::storage_blocks_genesis_check_applied_reducer;
use crate::storage::blocks::genesis::init::additional_data_put::storage_blocks_genesis_init_additional_data_put_reducer;
use crate::storage::blocks::genesis::init::commit_result_get::storage_blocks_genesis_init_commit_result_get_reducer;
use crate::storage::blocks::genesis::init::commit_result_put::storage_blocks_genesis_init_commit_result_put_reducer;
use crate::storage::blocks::genesis::init::header_put::storage_blocks_genesis_init_header_put_reducer;
use crate::storage::blocks::genesis::init::storage_blocks_genesis_init_reducer;
use crate::storage::request::storage_request_reducer;
use crate::storage::state_snapshot::create::storage_state_snapshot_create_reducer;
use crate::storage::{
    kv_block_additional_data::reducer as kv_block_additional_data_reducer,
    kv_block_header::reducer as kv_block_header_reducer,
    kv_block_meta::reducer as kv_block_meta_reducer, kv_constants::reducer as kv_constants_reducer,
    kv_cycle_eras::reducer as kv_cycle_eras_reducer,
    kv_cycle_meta::reducer as kv_cycle_meta_reducer,
    kv_operations::reducer as kv_operations_reducer,
};

pub fn last_action_reducer(state: &mut State, action: &ActionWithMeta) {
    state.set_last_action(action);
}

pub fn applied_actions_count_reducer(state: &mut State, _: &ActionWithMeta) {
    state.applied_actions_count += 1;
}

pub fn reducer(state: &mut State, action: &ActionWithMeta) {
    if let Action::StorageStateSnapshotCreateInit(_) = &action.action {
        // This action shouldn't cause changes in the state, so that in the
        // effects, we will save exact same state that was before calling
        // this action.
        return;
    }

    chain_reducers!(
        state,
        action,
        paused_loops_reducer,
        protocol_runner_current_head_reducer,
        protocol_runner_spawn_server_reducer,
        protocol_runner_init_reducer,
        protocol_runner_init_runtime_reducer,
        protocol_runner_init_context_reducer,
        protocol_runner_init_context_ipc_server_reducer,
        protocol_runner_reducer,
        current_head_reducer,
        block_applier_reducer,
        peer_reducer,
        peer_connection_outgoing_reducer,
        peer_connection_incoming_accept_reducer,
        peer_connection_incoming_reducer,
        peer_handshaking_reducer,
        peer_message_read_reducer,
        peer_message_write_reducer,
        peer_binary_message_write_reducer,
        peer_binary_message_read_reducer,
        peer_chunk_write_reducer,
        peer_chunk_read_reducer,
        peer_disconnection_reducer,
        peer_requests_potential_peers_get_reducer,
        peer_remote_requests_block_header_get_reducer,
        peer_remote_requests_block_operations_get_reducer,
        peer_remote_requests_current_branch_get_reducer,
        peers_dns_lookup_reducer,
        peers_add_multi_reducer,
        peers_add_reducer,
        peers_remove_reducer,
        peers_check_timeouts_reducer,
        peers_graylist_reducer,
        bootstrap_reducer,
        mempool_validator_reducer,
        mempool_reducer,
        rights_reducer,
        current_head_precheck_reducer,
        stats_current_head_reducer,
        prechecker_reducer,
        rpc_reducer,
        storage_request_reducer,
        storage_blocks_genesis_check_applied_reducer,
        storage_blocks_genesis_init_reducer,
        storage_blocks_genesis_init_header_put_reducer,
        storage_blocks_genesis_init_additional_data_put_reducer,
        storage_blocks_genesis_init_commit_result_get_reducer,
        storage_blocks_genesis_init_commit_result_put_reducer,
        storage_state_snapshot_create_reducer,
        kv_block_meta_reducer,
        kv_block_header_reducer,
        kv_block_additional_data_reducer,
        kv_constants_reducer,
        kv_cycle_eras_reducer,
        kv_cycle_meta_reducer,
        kv_operations_reducer,
        shutdown_reducer,
        // needs to be last!
        applied_actions_count_reducer,
        last_action_reducer
    );
}
