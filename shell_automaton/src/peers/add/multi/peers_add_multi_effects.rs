// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::peer::connection::outgoing::PeerConnectionOutgoingRandomInitAction;
use crate::{Action, ActionWithMeta, Service, Store};

pub fn peers_add_multi_effects<S: Service>(store: &mut Store<S>, action: &ActionWithMeta) {
    if let Action::PeersAddMulti(_) = &action.action {
        store.dispatch(PeerConnectionOutgoingRandomInitAction {});
    }
}
