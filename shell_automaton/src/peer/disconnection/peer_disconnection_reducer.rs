// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::peer::disconnection::PeerDisconnecting;
use crate::peer::PeerStatus;
use crate::{Action, ActionWithMeta, State};

pub fn peer_disconnection_reducer(state: &mut State, action: &ActionWithMeta) {
    match &action.action {
        Action::PeerDisconnect(action) => {
            let peer = match state.peers.get_mut(&action.address) {
                Some(v) => v,
                None => return,
            };

            peer.status = match &peer.status {
                PeerStatus::Potential => return,
                PeerStatus::Connecting(state) => {
                    if let Some(token) = state.token() {
                        PeerDisconnecting { token }.into()
                    } else {
                        PeerStatus::Disconnected
                    }
                }
                PeerStatus::Handshaking(state) => PeerDisconnecting { token: state.token }.into(),
                PeerStatus::Handshaked(state) => PeerDisconnecting { token: state.token }.into(),
                PeerStatus::Disconnecting(_) => return,
                PeerStatus::Disconnected => return,
            };
        }
        Action::PeerDisconnected(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                peer.status = match &peer.status {
                    PeerStatus::Potential => return,
                    PeerStatus::Connecting(_) => PeerStatus::Disconnected,
                    PeerStatus::Handshaking(_) => PeerStatus::Disconnected,
                    PeerStatus::Handshaked(_) => PeerStatus::Disconnected,
                    PeerStatus::Disconnecting(_) => PeerStatus::Disconnected,
                    PeerStatus::Disconnected => return,
                };
            }
        }
        _ => {}
    }
}
