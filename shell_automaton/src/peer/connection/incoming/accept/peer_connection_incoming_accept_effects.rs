// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::peers::add::PeersAddIncomingPeerAction;
use crate::service::mio_service::PeerConnectionIncomingAcceptError;
use crate::service::{MioService, Service};
use crate::{Action, ActionWithMeta, Store};

use super::{
    PeerConnectionIncomingAcceptAction, PeerConnectionIncomingAcceptErrorAction,
    PeerConnectionIncomingAcceptSuccessAction, PeerConnectionIncomingRejectedAction,
    PeerConnectionIncomingRejectedReason,
};

pub fn peer_connection_incoming_accept_effects<S>(store: &mut Store<S>, action: &ActionWithMeta)
where
    S: Service,
{
    match &action.action {
        Action::P2pServerEvent(_) => {
            store.dispatch(PeerConnectionIncomingAcceptAction {});
        }
        Action::PeerConnectionIncomingAccept(_) => {
            let state = store.state.get();

            match store.service.mio().peer_connection_incoming_accept() {
                Ok((peer_token, peer)) => {
                    let peer_address = peer.address;

                    if state.peers.connected_len() >= state.config.peers_connected_max {
                        store.dispatch(PeerConnectionIncomingRejectedAction {
                            token: peer_token,
                            address: peer_address,
                            reason:
                                PeerConnectionIncomingRejectedReason::PeersConnectedMaxBoundReached,
                        });
                        return;
                    }

                    if let Some(blacklisted) = state.peers.get_blacklisted_ip(&peer_address.ip()) {
                        let blacklisted = blacklisted.clone();

                        store.dispatch(PeerConnectionIncomingRejectedAction {
                            token: peer_token,
                            address: peer_address,
                            reason: PeerConnectionIncomingRejectedReason::PeerBlacklisted(
                                blacklisted,
                            ),
                        });
                        return;
                    }

                    store.dispatch(PeerConnectionIncomingAcceptSuccessAction {
                        token: peer_token,
                        address: peer_address,
                    })
                }
                Err(error) => store.dispatch(PeerConnectionIncomingAcceptErrorAction { error }),
            };
        }
        Action::PeerConnectionIncomingAcceptError(action) => {
            if !matches!(&action.error, PeerConnectionIncomingAcceptError::WouldBlock) {
                // if more progress can be made, accept next incoming connection.
                store.dispatch(PeerConnectionIncomingAcceptAction {});
            }
        }
        Action::PeerConnectionIncomingRejected(action) => {
            store.service.mio().peer_disconnect(action.token);
        }
        Action::PeerConnectionIncomingAcceptSuccess(action) => {
            store.dispatch(PeersAddIncomingPeerAction {
                token: action.token,
                address: action.address,
            });
            // there might be more connections in backlog. In mio we have
            // to exhaust those, or we won't receive another incoming
            // connection event, until we have new incoming connections.
            store.dispatch(PeerConnectionIncomingAcceptAction {});
        }
        _ => {}
    }
}
