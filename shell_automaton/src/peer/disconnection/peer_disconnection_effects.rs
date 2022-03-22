// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::peer::connection::outgoing::PeerConnectionOutgoingRandomInitAction;
use crate::peer::PeerStatus;
use crate::peers::remove::PeersRemoveAction;
use crate::service::actors_service::ActorsMessageTo;
use crate::service::{ActorsService, MioService, Service};
use crate::{Action, ActionWithMeta, Store};

use super::PeerDisconnectedAction;

pub fn peer_disconnection_effects<S>(store: &mut Store<S>, action: &ActionWithMeta)
where
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
                    store.dispatch(PeerDisconnectedAction { address });
                }
                PeerStatus::Disconnected => {
                    store.dispatch(PeerDisconnectedAction { address });
                }
                _ => {}
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

                    store.dispatch(PeersRemoveAction { address });
                    store.dispatch(PeerConnectionOutgoingRandomInitAction {});
                }
            }
        }
        _ => {}
    }
}
