// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::service::mio_service::PeerConnectionIncomingAcceptError;
use crate::{Action, ActionWithMeta, State};

use super::PeerConnectionIncomingAcceptState;

pub fn peer_connection_incoming_accept_reducer(state: &mut State, action: &ActionWithMeta) {
    let action_time = action.time_as_nanos();

    match &action.action {
        Action::PeerConnectionIncomingAcceptSuccess(action) => {
            match &state.peer_connection_incoming_accept {
                PeerConnectionIncomingAcceptState::Idle { .. } => {}
                _ => return,
            }
            state.peer_connection_incoming_accept = PeerConnectionIncomingAcceptState::Success {
                time: action_time,
                token: action.token,
                address: action.address,
            };
        }
        Action::PeerConnectionIncomingAcceptError(action) => {
            if matches!(&action.error, PeerConnectionIncomingAcceptError::WouldBlock) {
                return;
            }
            state.peer_connection_incoming_accept = PeerConnectionIncomingAcceptState::Error {
                time: action_time,
                error: action.error.clone(),
            };
        }
        Action::PeerConnectionIncomingRejected(action) => {
            match &state.peer_connection_incoming_accept {
                PeerConnectionIncomingAcceptState::Idle { .. } => {}
                _ => return,
            }
            state.peer_connection_incoming_accept = PeerConnectionIncomingAcceptState::Rejected {
                time: action_time,
                token: action.token,
                address: action.address,
                reason: action.reason.clone(),
            };
        }
        _ => {}
    }
}
