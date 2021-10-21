use redux_rs::{ActionWithId, Store};

use crate::peers::add::PeersAddIncomingPeerAction;
use crate::service::mio_service::PeerConnectionIncomingAcceptError;
use crate::service::{MioService, Service};
use crate::{action::Action, State};

use super::{
    PeerConnectionIncomingAcceptAction, PeerConnectionIncomingAcceptErrorAction,
    PeerConnectionIncomingAcceptSuccessAction, PeerConnectionIncomingRejectedAction,
    PeerConnectionIncomingRejectedReason,
};

pub fn peer_connection_incoming_accept_effects<S>(
    store: &mut Store<State, S, Action>,
    action: &ActionWithId<Action>,
) where
    S: Service,
{
    match &action.action {
        Action::P2pServerEvent(_) => {
            store.dispatch(PeerConnectionIncomingAcceptAction {}.into());
        }
        Action::PeerConnectionIncomingAccept(_) => {
            let state = store.state.get();

            match store.service.mio().peer_connection_incoming_accept() {
                Ok((peer_token, peer)) => {
                    let peer_address = peer.address;

                    if state.peers.connected_len() >= state.config.peers_connected_max {
                        return store.dispatch(PeerConnectionIncomingRejectedAction {
                            token: peer_token,
                            address: peer_address,
                            reason:
                                PeerConnectionIncomingRejectedReason::PeersConnectedMaxBoundReached,
                        }.into());
                    }

                    if let Some(blacklisted) = state.peers.get_blacklisted_ip(&peer_address.ip()) {
                        let blacklisted = blacklisted.clone();

                        return store.dispatch(
                            PeerConnectionIncomingRejectedAction {
                                token: peer_token,
                                address: peer_address,
                                reason: PeerConnectionIncomingRejectedReason::PeerBlacklisted(
                                    blacklisted,
                                ),
                            }
                            .into(),
                        );
                    }

                    store.dispatch(
                        PeerConnectionIncomingAcceptSuccessAction {
                            token: peer_token,
                            address: peer_address,
                        }
                        .into(),
                    );
                }
                Err(error) => {
                    store.dispatch(PeerConnectionIncomingAcceptErrorAction { error }.into())
                }
            }
        }
        Action::PeerConnectionIncomingAcceptError(action) => {
            if !matches!(&action.error, PeerConnectionIncomingAcceptError::WouldBlock) {
                // if more progress can be made, accept next incoming connection.
                store.dispatch(PeerConnectionIncomingAcceptAction {}.into());
            }
        }
        Action::PeerConnectionIncomingRejected(action) => {
            store.service.mio().peer_disconnect(action.token);
        }
        Action::PeerConnectionIncomingAcceptSuccess(action) => {
            store.dispatch(
                PeersAddIncomingPeerAction {
                    token: action.token,
                    address: action.address,
                }
                .into(),
            );
            // there might be more connections in backlog. In mio we have
            // to exhaust those, or we won't receive another incoming
            // connection event, until we have new incoming connections.
            store.dispatch(PeerConnectionIncomingAcceptAction {}.into());
        }
        _ => {}
    }
}
