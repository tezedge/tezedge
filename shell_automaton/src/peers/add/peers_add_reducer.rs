// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::peer::connection::incoming::PeerConnectionIncomingState;
use crate::peer::{Peer, PeerIOLoopState, PeerStatus};
use crate::{Action, ActionWithMeta, State};

use super::PeersAddIncomingPeerAction;

pub fn peers_add_reducer(state: &mut State, action: &ActionWithMeta) {
    if let Action::PeersAddIncomingPeer(PeersAddIncomingPeerAction { address, token }) =
        &action.action
    {
        if let Ok(entry) = state.peers.entry(*address) {
            entry.or_insert_with(|| Peer {
                status: PeerStatus::Connecting(
                    PeerConnectionIncomingState::Pending {
                        time: action.time_as_nanos(),
                        token: *token,
                    }
                    .into(),
                ),
                try_read_loop: PeerIOLoopState::Idle,
                try_write_loop: PeerIOLoopState::Idle,
            });
        }
    }
}
