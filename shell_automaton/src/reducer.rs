use redux_rs::{chain_reducers, ActionWithId};

use crate::action::Action;
use crate::State;

use crate::peer::connection::incoming::accept::peer_connection_incoming_accept_reducer;
use crate::peer::connection::incoming::peer_connection_incoming_reducer;
use crate::peer::connection::outgoing::peer_connection_outgoing_reducer;
use crate::peer::disconnection::peer_disconnection_reducer;
use crate::peer::handshaking::connection_message::read::peer_connection_message_read_reducer;
use crate::peer::handshaking::connection_message::write::peer_connection_message_write_reducer;
use crate::peer::handshaking::peer_handshaking_reducer;

use crate::peers::add::multi::peers_add_multi_reducer;
use crate::peers::add::peers_add_reducer;
use crate::peers::dns_lookup::peers_dns_lookup_reducer;
use crate::peers::remove::peers_remove_reducer;

use crate::storage::block_header::put::storage_block_header_put_reducer;
use crate::storage::request::storage_request_reducer;
use crate::storage::state_snapshot::create::storage_state_snapshot_create_reducer;

pub fn last_action_id_reducer(state: &mut State, action: &ActionWithId<Action>) {
    state.last_action_id = action.id;
}

pub fn reducer(state: &mut State, action: &ActionWithId<Action>) {
    chain_reducers!(
        state,
        action,
        // needs to be first!
        storage_state_snapshot_create_reducer,
        peers_dns_lookup_reducer,
        peers_add_multi_reducer,
        peers_add_reducer,
        peers_remove_reducer,
        peer_connection_outgoing_reducer,
        peer_connection_incoming_accept_reducer,
        peer_connection_incoming_reducer,
        peer_handshaking_reducer,
        peer_connection_message_write_reducer,
        peer_connection_message_read_reducer,
        peer_disconnection_reducer,
        storage_block_header_put_reducer,
        storage_request_reducer,
        // needs to be last!
        last_action_id_reducer
    );
}
