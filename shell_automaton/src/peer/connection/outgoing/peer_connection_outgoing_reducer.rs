// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use redux_rs::ActionWithId;

use crate::peer::connection::PeerConnectionState;
use crate::peer::PeerStatus;
use crate::{Action, State};

use super::PeerConnectionOutgoingState;

pub fn peer_connection_outgoing_reducer(state: &mut State, action: &ActionWithId<Action>) {
    let action_time = action.time_as_nanos();

    match &action.action {
        Action::PeerConnectionOutgoingInit(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                if matches!(peer.status, PeerStatus::Potential) {
                    peer.status = PeerStatus::Connecting(
                        PeerConnectionOutgoingState::Idle { time: action_time }.into(),
                    );
                }
            }
        }
        Action::PeerConnectionOutgoingPending(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                if matches!(
                    peer.status,
                    PeerStatus::Connecting(PeerConnectionState::Outgoing(
                        PeerConnectionOutgoingState::Idle { .. }
                    ))
                ) {
                    peer.status = PeerStatus::Connecting(
                        PeerConnectionOutgoingState::Pending {
                            time: action_time,
                            token: action.token,
                        }
                        .into(),
                    );
                }
            }
        }
        Action::PeerConnectionOutgoingError(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                let token = match peer.status {
                    PeerStatus::Connecting(PeerConnectionState::Outgoing(
                        PeerConnectionOutgoingState::Idle { .. },
                    )) => None,
                    PeerStatus::Connecting(PeerConnectionState::Outgoing(
                        PeerConnectionOutgoingState::Pending { token, .. },
                    )) => Some(token),
                    _ => return,
                };
                peer.status = PeerStatus::Connecting(
                    PeerConnectionOutgoingState::Error {
                        time: action_time,
                        token,
                        error: action.error.clone(),
                    }
                    .into(),
                );
            }
        }
        Action::PeerConnectionOutgoingSuccess(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                if let PeerStatus::Connecting(PeerConnectionState::Outgoing(
                    PeerConnectionOutgoingState::Pending {
                        time: action_time,
                        token,
                    },
                )) = peer.status
                {
                    peer.status = PeerStatus::Connecting(
                        PeerConnectionOutgoingState::Success {
                            time: action_time,
                            token,
                        }
                        .into(),
                    );
                }
            }
        }
        _ => {}
    }
}
