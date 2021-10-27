// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use redux_rs::ActionWithId;

use crate::peer::connection::incoming::PeerConnectionIncomingState;
use crate::peer::{Peer, PeerIOLoopState, PeerQuota, PeerStatus};
use crate::{Action, State};

use super::PeersAddIncomingPeerAction;

pub fn peers_add_reducer(state: &mut State, action: &ActionWithId<Action>) {
    match &action.action {
        Action::PeersAddIncomingPeer(PeersAddIncomingPeerAction { address, token }) => {
            if let Ok(entry) = state.peers.entry(*address) {
                entry.or_insert_with(|| Peer {
                    status: PeerStatus::Connecting(
                        PeerConnectionIncomingState::Pending {
                            time: action.time_as_nanos(),
                            token: *token,
                        }
                        .into(),
                    ),
                    quota: PeerQuota::new(action.id),
                    try_read_loop: PeerIOLoopState::Idle,
                    try_write_loop: PeerIOLoopState::Idle,
                });
            }
        }
        _ => {}
    }
}
