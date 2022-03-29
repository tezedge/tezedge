// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::peer::disconnection::PeerDisconnectAction;
use crate::peers::graylist::{PeerGraylistReason, PeersGraylistAddressAction};
use crate::service::Service;
use crate::{Action, ActionWithMeta, Store};

pub fn peer_connection_closed_effects<S>(store: &mut Store<S>, action: &ActionWithMeta)
where
    S: Service,
{
    if let Action::PeerConnectionClosed(action) = &action.action {
        store.dispatch(PeerDisconnectAction {
            address: action.address,
        });
        // store.dispatch(PeersGraylistAddressAction {
        //     address: action.address,
        //     reason: PeerGraylistReason::ConnectionClosed,
        // });
    }
}
