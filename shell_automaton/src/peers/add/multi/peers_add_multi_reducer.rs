// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::peer::{Peer, PeerIOLoopState, PeerStatus};
use crate::{Action, ActionWithMeta, State};

use super::PeersAddMultiAction;

pub fn peers_add_multi_reducer(state: &mut State, action: &ActionWithMeta) {
    match &action.action {
        Action::PeersAddMulti(PeersAddMultiAction { addresses }) => {
            let max_len = state
                .config
                .peers_potential_max
                .saturating_sub(state.peers.potential_len());

            for address in addresses.iter().take(max_len) {
                if let Ok(entry) = state.peers.entry(*address) {
                    entry.or_insert_with(|| Peer {
                        status: PeerStatus::Potential,
                        try_read_loop: PeerIOLoopState::Idle,
                        try_write_loop: PeerIOLoopState::Idle,
                    });
                }
            }
        }
        _ => {}
    }
}
