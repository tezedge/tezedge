// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::service::mio_service::{PeerConnectionIncomingAcceptError, WebSocketIncomingAcceptError, MioWsConnection};
use crate::service::{MioService, Service};
use crate::{Action, ActionWithMeta, Store};

use super::{
    WebSocketConnectionIncomingAcceptAction, WebSocketConnectionIncomingAcceptSuccessAction,
    WebSocketConnectionIncomingAcceptErrorAction, WebSocketConnectionHandshakeContinueAction
};

pub fn websocket_connection_incoming_accept_effects<S>(store: &mut Store<S>, action: &ActionWithMeta)
where
    S: Service
{
    match &action.action {
        Action::WebsocketServerEvent(_) => {
            store.dispatch(WebSocketConnectionIncomingAcceptAction {});
        }
        Action::WebsocketClientEvent(event) => {
            if let Some(client) = store.service.mio().websocket_client_get(event.token) {
                if client.connection.is_established() {
                    // TODO: dispatch a read action
                } else {
                    store.dispatch(WebSocketConnectionHandshakeContinueAction {
                        token: event.token
                    });
                }
            }
        }
        Action::WebSocketConnectionIncomingAccept(_) => {
            match store.service.mio().websocket_connection_incoming_accept() {
                // TODO: probobly some congestion checks
                Ok((peer_token, is_handshake_complete)) => {
                    if is_handshake_complete {
                        store.dispatch(WebSocketConnectionIncomingAcceptSuccessAction {
                            token: peer_token,
                        });
                    }
                }
                Err(error) => {
                    store.dispatch(WebSocketConnectionIncomingAcceptErrorAction { error });
                },
            };
        }
        Action::WebSocketConnectionIncomingAcceptError(action) => {
            if !matches!(&action.error, WebSocketIncomingAcceptError::ConnectionError(PeerConnectionIncomingAcceptError::WouldBlock)) {
                // if no more progress can be made, accept next incoming connection.
                store.dispatch(WebSocketConnectionIncomingAcceptAction {});
            }
        }
        Action::WebsocketConnectionContinueHandshake(action) => {
            match store.service.mio().websocket_connection_continue_handshaking(action.token) {
                Ok((peer_token, is_handshake_complete)) => {
                    if is_handshake_complete {
                        store.dispatch(WebSocketConnectionIncomingAcceptSuccessAction {
                            token: peer_token,
                        });
                    }
                }
                Err(error) => {
                    store.dispatch(WebSocketConnectionIncomingAcceptErrorAction { error });
                },
            }
        }
        _ => {}
    }
}