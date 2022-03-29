// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::peer::handshaking::PeerHandshakingInitAction;
use crate::peers::graylist::{PeerGraylistReason, PeersGraylistAddressAction};
use crate::service::Service;
use crate::{Action, ActionWithMeta, Store};

pub fn peer_connection_incoming_effects<S>(store: &mut Store<S>, action: &ActionWithMeta)
where
    S: Service,
{
    match &action.action {
        Action::PeerConnectionIncomingSuccess(action) => {
            store.dispatch(PeerHandshakingInitAction {
                address: action.address,
            });
        }
        Action::PeerConnectionIncomingError(action) => {
            store.dispatch(PeersGraylistAddressAction {
                address: action.address,
                reason: PeerGraylistReason::ConnectionIncomingError,
            });
        }
        _ => {}
    }
}
