// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use redux_rs::chain_reducers;

use crate::action::{Action, ActionWithMeta};
use crate::State;

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

use crate::peers::add::multi::peers_add_multi_reducer;
use crate::peers::add::peers_add_reducer;
use crate::peers::check::timeouts::peers_check_timeouts_reducer;
use crate::peers::dns_lookup::peers_dns_lookup_reducer;
use crate::peers::graylist::peers_graylist_reducer;
use crate::peers::remove::peers_remove_reducer;

use crate::mempool::mempool_reducer;
use crate::protocol::protocol_reducer;

use crate::prechecker::prechecker_reducer;
use crate::rights::rights_reducer;

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
    match &action.action {
        Action::StorageStateSnapshotCreateInit(_) => {
            // This action shouldn't cause changes in the state, so that in the
            // effects, we will save exact same state that was before calling
            // this action.
            return;
        }
        _ => {}
    }

    chain_reducers!(
        state,
        action,
        paused_loops_reducer,
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
        peers_dns_lookup_reducer,
        peers_add_multi_reducer,
        peers_add_reducer,
        peers_remove_reducer,
        peers_check_timeouts_reducer,
        peers_graylist_reducer,
        mempool_reducer,
        protocol_reducer,
        rights_reducer,
        prechecker_reducer,
        storage_request_reducer,
        storage_state_snapshot_create_reducer,
        kv_block_meta_reducer,
        kv_block_header_reducer,
        kv_block_additional_data_reducer,
        kv_constants_reducer,
        kv_cycle_eras_reducer,
        kv_cycle_meta_reducer,
        kv_operations_reducer,
        // needs to be last!
        applied_actions_count_reducer,
        last_action_reducer
    );
}
