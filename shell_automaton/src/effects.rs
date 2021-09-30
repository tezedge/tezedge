use redux_rs::{ActionWithId, Store};

use crate::action::Action;
use crate::peer::connection::incoming::accept::peer_connection_incoming_accept_effects;
use crate::peer::connection::incoming::peer_connection_incoming_effects;
use crate::service::storage_service::{StorageRequest, StorageRequestPayload};
use crate::service::{Service, StorageService};
use crate::State;

use crate::peer::connection::outgoing::peer_connection_outgoing_effects;
use crate::peer::disconnection::peer_disconnection_effects;
use crate::peer::handshaking::connection_message::read::peer_connection_message_read_effects;
use crate::peer::handshaking::connection_message::write::peer_connection_message_write_effects;
use crate::peer::handshaking::peer_handshaking_effects;
use crate::peer::peer_effects;

use crate::peers::add::multi::peers_add_multi_effects;
use crate::peers::dns_lookup::peers_dns_lookup_effects;

use crate::storage::block_header::put::storage_block_header_put_effects;
use crate::storage::request::storage_request_effects;
use crate::storage::state_snapshot::create::{
    storage_state_snapshot_create_effects, StorageStateSnapshotCreateAction,
};

use crate::rpc::rpc_effects;

fn log_effects<S: Service>(store: &mut Store<State, S, Action>, action: &ActionWithId<Action>) {
    eprintln!("[+] Action: {:#?}", &action);
    // eprintln!("[+] State: {:#?}\n", store.state());
}

fn last_action_effects<S: Service>(
    store: &mut Store<State, S, Action>,
    action: &ActionWithId<Action>,
) {
    let last_action_id_num: u64 = store.state().last_action_id.into();
    if last_action_id_num % 10000 == 0 {
        store.dispatch(StorageStateSnapshotCreateAction {}.into());
    }
    store.service.storage().request_send(StorageRequest {
        id: None,
        payload: StorageRequestPayload::ActionPut(Box::new(action.clone())),
    });
}

pub fn effects<S: Service>(store: &mut Store<State, S, Action>, action: &ActionWithId<Action>) {
    // these two effects must be first!
    log_effects(store, action);
    last_action_effects(store, action);

    storage_state_snapshot_create_effects(store, action);

    peers_dns_lookup_effects(store, action);
    peers_add_multi_effects(store, action);

    peer_effects(store, action);
    peer_connection_outgoing_effects(store, action);
    peer_connection_incoming_accept_effects(store, action);
    peer_connection_incoming_effects(store, action);
    peer_handshaking_effects(store, action);
    peer_connection_message_write_effects(store, action);
    peer_connection_message_read_effects(store, action);
    peer_disconnection_effects(store, action);

    storage_block_header_put_effects(store, action);
    storage_request_effects(store, action);

    rpc_effects(store, action);
}
