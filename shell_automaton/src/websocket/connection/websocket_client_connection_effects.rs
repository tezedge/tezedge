// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::thread::AccessError;

use crate::peers::add::PeersAddIncomingPeerAction;
use crate::service::mio_service::PeerConnectionIncomingAcceptError;
use crate::service::{MioService, Service};
use crate::{Action, ActionWithMeta, Store};

use super::{
    WebSocketConnectionIncomingAcceptAction, WebSocketConnectionIncomingAcceptSuccessAction,
    WebSocketConnectionIncomingAcceptErrorAction,
};

pub fn websocket_connection_incoming_accept_effects<S>(store: &mut Store<S>, action: &ActionWithMeta)
where
    S: Service
{
    match &action.action {
        Action::WebsocketServerEvent(_) => {
            store.dispatch(WebSocketConnectionIncomingAcceptAction {});
        }
        Action::WebSocketConnectionIncomingAccept(_) => {
            let state = store.state.get();

            match store.service.mio().websocket_connection_incoming_accept() {
                // TODO: probobly some congestion checks
                Ok((client_token, address)) => {
                    store.dispatch(WebSocketConnectionIncomingAcceptSuccessAction {
                        token: client_token,
                        address
                    })
                }
                Err(error) => store.dispatch(WebSocketConnectionIncomingAcceptErrorAction { error }),
            };
        }
        Action::WebSocketConnectionIncomingAcceptError(action) => {
            if !matches!(&action.error, PeerConnectionIncomingAcceptError::WouldBlock) {
                // if no more progress can be made, accept next incoming connection.
                store.dispatch(WebSocketConnectionIncomingAcceptAction {});
            }
        }
        _ => {}
    }
}