// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::peers::add::multi::PeersAddMultiAction;
use crate::service::{DnsService, Service};
use crate::{action::Action, ActionWithMeta, Store};

use super::{
    PeersDnsLookupCleanupAction, PeersDnsLookupErrorAction, PeersDnsLookupStatus,
    PeersDnsLookupSuccessAction,
};

pub fn peers_dns_lookup_effects<S: Service>(store: &mut Store<S>, action: &ActionWithMeta) {
    match &action.action {
        Action::PeersDnsLookupInit(action) => {
            let result = store
                .service()
                .dns()
                .resolve_dns_name_to_peer_address(&action.address, action.port);
            match result {
                Ok(addresses) => store.dispatch(PeersDnsLookupSuccessAction { addresses }),
                Err(err) => store.dispatch(PeersDnsLookupErrorAction { error: err.into() }),
            };
        }
        Action::PeersDnsLookupSuccess(_) => {
            let dns_lookup_state = match store.state.get().peers.dns_lookup.as_ref() {
                Some(v) => v,
                None => return,
            };
            match &dns_lookup_state.status {
                PeersDnsLookupStatus::Success { addresses } => {
                    let addresses = addresses.clone();
                    store.dispatch(PeersAddMultiAction { addresses });
                }
                _ => {}
            }
            store.dispatch(PeersDnsLookupCleanupAction {});
        }
        _ => {}
    }
}
