// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use redux_rs::ActionWithId;

use crate::peer::connection::PeerConnectionState;
use crate::{action::Action, peer::PeerStatus, State};

use super::PeerConnectionIncomingState;

pub fn peer_connection_incoming_reducer(state: &mut State, action: &ActionWithId<Action>) {
    let action_time = action.time_as_nanos();

    match &action.action {
        Action::PeerConnectionIncomingError(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                if let PeerStatus::Connecting(PeerConnectionState::Incoming(incoming)) =
                    &peer.status
                {
                    peer.status = PeerStatus::Connecting(
                        PeerConnectionIncomingState::Error {
                            time: action_time,
                            error: action.error.clone(),
                            token: incoming.token(),
                        }
                        .into(),
                    );
                }
            }
        }
        Action::PeerConnectionIncomingSuccess(action) => {
            let peers_connected = state.peers.connected_len();
            if let Some(peer) = state.peers.get_mut(&action.address) {
                if let PeerStatus::Connecting(PeerConnectionState::Incoming(
                    PeerConnectionIncomingState::Pending { token, .. },
                )) = peer.status
                {
                    if peers_connected < state.config.peers_connected_max {
                        peer.status = PeerStatus::Connecting(
                            PeerConnectionIncomingState::Success {
                                time: action_time,
                                token,
                            }
                            .into(),
                        );
                    }
                }
            }
        }
        _ => {}
    }
}
