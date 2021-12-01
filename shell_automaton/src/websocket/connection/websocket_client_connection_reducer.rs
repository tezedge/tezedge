// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::service::mio_service::{PeerConnectionIncomingAcceptError, WebSocketIncomingAcceptError};
use crate::{Action, ActionWithMeta, State};

// TODO: create a custom state for the websocket? rename to TcpConnectionIncomingState?
use super::WebSocketConnectionIncomingAcceptState;

pub fn websocket_connection_incoming_accept_reducer(state: &mut State, action: &ActionWithMeta) {
    let action_time = action.time_as_nanos();

    // TODO
    match &action.action {
        Action::WebSocketConnectionIncomingAcceptSuccess(action) => {
            match &state.websocket_connection_incoming_accept {
                WebSocketConnectionIncomingAcceptState::Idle { .. } => {}
                _ => return,
            }
            state.websocket_connection_incoming_accept = WebSocketConnectionIncomingAcceptState::Success {
                time: action_time,
                token: action.token,
            };
        }
        Action::WebSocketConnectionIncomingAcceptError(action) => {
            if matches!(&action.error, WebSocketIncomingAcceptError::ConnectionError(PeerConnectionIncomingAcceptError::WouldBlock)) {
                return;
            }
            state.websocket_connection_incoming_accept = WebSocketConnectionIncomingAcceptState::Error {
                time: action_time,
                error: action.error.clone(),
            };
        }
        _ => {}
    }
}