// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use redux_rs::{ActionWithId, Store};

use crate::peer::handshaking::PeerHandshakingInitAction;
use crate::peers::graylist::PeersGraylistAddressAction;
use crate::service::Service;
use crate::{action::Action, State};

pub fn peer_connection_incoming_effects<S>(
    store: &mut Store<State, S, Action>,
    action: &ActionWithId<Action>,
) where
    S: Service,
{
    match &action.action {
        Action::PeerConnectionIncomingSuccess(action) => store.dispatch(
            PeerHandshakingInitAction {
                address: action.address,
            }
            .into(),
        ),
        Action::PeerConnectionIncomingError(action) => store.dispatch(
            PeersGraylistAddressAction {
                address: action.address,
            }
            .into(),
        ),
        _ => {}
    }
}
