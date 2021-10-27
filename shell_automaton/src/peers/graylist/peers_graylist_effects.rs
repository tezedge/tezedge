// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use redux_rs::{ActionWithId, Store};

use crate::peer::disconnection::PeerDisconnectAction;
use crate::peer::PeerStatus;
use crate::peers::remove::PeersRemoveAction;
use crate::service::Service;
use crate::{Action, State};

use super::{PeersGraylistIpAddAction, PeersGraylistIpAddedAction, PeersGraylistIpRemovedAction};

pub fn peers_graylist_effects<S: Service>(
    store: &mut Store<State, S, Action>,
    action: &ActionWithId<Action>,
) {
    match &action.action {
        Action::PeersGraylistAddress(action) => {
            if store.state.get().config.peers_graylist_disable {
                return store.dispatch(
                    PeerDisconnectAction {
                        address: action.address,
                    }
                    .into(),
                );
            }

            store.dispatch(
                PeersGraylistIpAddAction {
                    ip: action.address.ip(),
                }
                .into(),
            );
        }
        Action::PeersGraylistIpAdd(action) => {
            store.dispatch(PeersGraylistIpAddedAction { ip: action.ip }.into());
        }
        Action::PeersGraylistIpAdded(action) => {
            let peers = &store.state.get().peers;
            // find all peers with same ip and disconnect/remove them.
            let mut remove_peers = Vec::with_capacity(16);
            let mut disconnect_peers = Vec::with_capacity(16);

            peers
                .iter()
                .filter(|(addr, _)| addr.ip().eq(&action.ip))
                .for_each(|(addr, peer)| match &peer.status {
                    PeerStatus::Potential => remove_peers.push(*addr),

                    PeerStatus::Connecting(_)
                    | PeerStatus::Handshaking(_)
                    | PeerStatus::Handshaked(_) => disconnect_peers.push(*addr),

                    PeerStatus::Disconnecting(_) => {}
                    PeerStatus::Disconnected => {}
                });

            for address in remove_peers {
                store.dispatch(PeersRemoveAction { address }.into());
            }

            for address in disconnect_peers {
                store.dispatch(PeerDisconnectAction { address }.into());
            }
        }
        Action::PeersGraylistIpRemove(action) => {
            store.dispatch(PeersGraylistIpRemovedAction { ip: action.ip }.into());
        }
        _ => {}
    }
}
