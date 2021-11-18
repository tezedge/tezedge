// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::peer::disconnection::PeerDisconnectAction;
use crate::peer::PeerStatus;
use crate::peers::remove::PeersRemoveAction;
use crate::{Action, ActionWithMeta, Service, Store};

use super::{PeersGraylistIpAddAction, PeersGraylistIpAddedAction, PeersGraylistIpRemovedAction};

pub fn peers_graylist_effects<S: Service>(store: &mut Store<S>, action: &ActionWithMeta) {
    match &action.action {
        Action::PeersGraylistAddress(action) => {
            if store.state.get().config.peers_graylist_disable {
                store.dispatch(PeerDisconnectAction {
                    address: action.address,
                });
                return;
            }

            store.dispatch(PeersGraylistIpAddAction {
                ip: action.address.ip(),
            });
        }
        Action::PeersGraylistIpAdd(action) => {
            store.dispatch(PeersGraylistIpAddedAction { ip: action.ip });
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
                store.dispatch(PeersRemoveAction { address });
            }

            for address in disconnect_peers {
                store.dispatch(PeerDisconnectAction { address });
            }
        }
        Action::PeersGraylistIpRemove(action) => {
            store.dispatch(PeersGraylistIpRemovedAction { ip: action.ip });
        }
        _ => {}
    }
}
