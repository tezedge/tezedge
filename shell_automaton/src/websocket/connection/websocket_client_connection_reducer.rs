// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::service::mio_service::PeerConnectionIncomingAcceptError;
use crate::{Action, ActionWithMeta, State};

// TODO: create a custom state for the websocket? rename to TcpConnectionIncomingState?
use crate::peer::connection::incoming::accept::PeerConnectionIncomingAcceptState;

pub fn websocket_connection_incoming_accept_reducer(state: &mut State, action: &ActionWithMeta) {
    let action_time = action.time_as_nanos();

    // TODO
    match &action.action {
        Action::WebSocketConnectionIncomingAcceptSuccess(action) => {
            match &state.websocket_connection_incoming_accept {
                PeerConnectionIncomingAcceptState::Idle { .. } => {}
                _ => return,
            }
            state.websocket_connection_incoming_accept = PeerConnectionIncomingAcceptState::Success {
                time: action_time,
                token: action.token,
                address: action.address,
            };
        }
        Action::WebSocketConnectionIncomingAcceptError(action) => {
            if matches!(&action.error, PeerConnectionIncomingAcceptError::WouldBlock) {
                return;
            }
            state.peer_connection_incoming_accept = PeerConnectionIncomingAcceptState::Error {
                time: action_time,
                error: action.error.clone(),
            };
        }
        _ => {}
    }
}