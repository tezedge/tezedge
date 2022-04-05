// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::{Action, ActionWithMeta, State};

use super::PeerRequestsPotentialPeersGetState;

pub fn peer_requests_potential_peers_get_reducer(state: &mut State, action: &ActionWithMeta) {
    match &action.action {
        Action::PeerRequestsPotentialPeersGetInit(content) => {
            let peer = match state.peers.get_handshaked_mut(&content.address) {
                Some(v) => v,
                None => return,
            };
            peer.requests.potential_peers_get = PeerRequestsPotentialPeersGetState::Init {
                time: action.time_as_nanos(),
            };
        }
        Action::PeerRequestsPotentialPeersGetPending(content) => {
            let peer = match state.peers.get_handshaked_mut(&content.address) {
                Some(v) => v,
                None => return,
            };
            peer.requests.potential_peers_get = PeerRequestsPotentialPeersGetState::Pending {
                time: action.time_as_nanos(),
            };
        }
        Action::PeerRequestsPotentialPeersGetError(content) => {
            let peer = match state.peers.get_handshaked_mut(&content.address) {
                Some(v) => v,
                None => return,
            };
            peer.requests.potential_peers_get = PeerRequestsPotentialPeersGetState::Error {
                time: action.time_as_nanos(),
                error: content.error.clone(),
            };
        }
        Action::PeerRequestsPotentialPeersGetSuccess(content) => {
            let peer = match state.peers.get_handshaked_mut(&content.address) {
                Some(v) => v,
                None => return,
            };
            peer.requests.potential_peers_get = PeerRequestsPotentialPeersGetState::Success {
                time: action.time_as_nanos(),
                result: content.result.clone(),
            };
        }
        Action::PeerRequestsPotentialPeersGetFinish(content) => {
            let peer = match state.peers.get_handshaked_mut(&content.address) {
                Some(v) => v,
                None => return,
            };
            peer.requests.potential_peers_get = PeerRequestsPotentialPeersGetState::Idle {
                time: action.time_as_nanos(),
            };
        }
        _ => {}
    }
}
