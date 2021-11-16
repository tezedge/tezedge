// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use redux_rs::{ActionWithId, Store};

use crate::peer::connection::outgoing::PeerConnectionOutgoingRandomInitAction;
use crate::peer::PeerStatus;
use crate::peers::remove::PeersRemoveAction;
use crate::service::actors_service::ActorsMessageTo;
use crate::service::{ActorsService, MioService, Service};
use crate::{action::Action, State};

use super::PeerDisconnectedAction;

pub fn peer_disconnection_effects<S>(
    store: &mut Store<State, S, Action>,
    action: &ActionWithId<Action>,
) where
    S: Service,
{
    match &action.action {
        Action::PeerDisconnect(action) => {
            let address = action.address;
            let peer = match store.state.get().peers.get(&address) {
                Some(v) => v,
                None => return,
            };

            match &peer.status {
                PeerStatus::Disconnecting(disconnection_state) => {
                    let peer_token = disconnection_state.token;
                    store.service().mio().peer_disconnect(peer_token);
                    store.dispatch(PeerDisconnectedAction { address }.into());
                }
                PeerStatus::Disconnected => {
                    store.dispatch(PeerDisconnectedAction { address }.into());
                }
                _ => return,
            };
        }
        Action::PeerDisconnected(action) => {
            if let Some(peer) = store.state.get().peers.get(&action.address) {
                if matches!(&peer.status, PeerStatus::Disconnected) {
                    let address = action.address;

                    store
                        .service
                        .actors()
                        .send(ActorsMessageTo::PeerDisconnected(address));

                    store.dispatch(PeersRemoveAction { address }.into());
                    store.dispatch(PeerConnectionOutgoingRandomInitAction {}.into());
                }
            }
        }
        _ => {}
    }
}
