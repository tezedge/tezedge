// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::peer::connection::outgoing::PeerConnectionOutgoingRandomInitAction;
use crate::peers::dns_lookup::PeersDnsLookupInitAction;
use crate::service::Service;
use crate::{Action, ActionWithMeta, Store};

pub fn peers_init_effects<S: Service>(store: &mut Store<S>, action: &ActionWithMeta) {
    if let Action::PeersInit(_) = &action.action {
        let list = store.state().config.peers_dns_lookup_addresses.clone();

        // Do dns lookups to gather some potential peers.
        for (address, port) in list.into_iter() {
            store.dispatch(PeersDnsLookupInitAction { address, port });
        }

        // Try connecting to potential peers if we need peers.
        store.dispatch(PeerConnectionOutgoingRandomInitAction {});
    }
}
