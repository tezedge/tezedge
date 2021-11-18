// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::peer::PeerStatus;
use crate::{Action, ActionWithMeta, State};

pub fn peers_remove_reducer(state: &mut State, action: &ActionWithMeta) {
    match &action.action {
        Action::PeersRemove(action) => {
            if let Some(peer) = state.peers.get(&action.address) {
                // we aren't allowed to remove peer until peer is disconnected.
                if matches!(&peer.status, PeerStatus::Disconnected) {
                    state.peers.remove(&action.address);
                }
            }
        }
        _ => {}
    }
}
