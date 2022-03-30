// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::peer::handshaking::PeerHandshakingInitAction;
use crate::peers::graylist::{PeerGraylistReason, PeersGraylistAddressAction};
use crate::service::{MioService, RandomnessService, Service};
use crate::{Action, ActionWithMeta, Store};

use super::{
    PeerConnectionOutgoingErrorAction, PeerConnectionOutgoingInitAction,
    PeerConnectionOutgoingPendingAction, PeerConnectionOutgoingRandomInitAction,
};

pub fn peer_connection_outgoing_effects<S>(store: &mut Store<S>, action: &ActionWithMeta)
where
    S: Service,
{
    match &action.action {
        Action::PeerConnectionOutgoingRandomInit(_) => {
            let state = store.state.get();
            let potential_peers = state.peers.potential_iter().collect::<Vec<_>>();

            if state.peers.connected_len() >= state.config.peers_connected_max {
                return;
            }

            if let Some(address) = store.service.randomness().choose_peer(&potential_peers) {
                store.dispatch(PeerConnectionOutgoingInitAction { address });
            }
        }
        Action::PeerConnectionOutgoingInit(action) => {
            let address = action.address;
            let result = store.service().mio().peer_connection_init(address);
            match result {
                Ok(token) => store.dispatch(PeerConnectionOutgoingPendingAction { address, token }),
                Err(error) => store.dispatch(PeerConnectionOutgoingErrorAction {
                    address,
                    error: error.into(),
                }),
            };
        }
        Action::PeerConnectionOutgoingPending(_) => {
            // try to connect to next random peer.
            store.dispatch(PeerConnectionOutgoingRandomInitAction {});
        }
        Action::PeerConnectionOutgoingSuccess(action) => {
            store.dispatch(PeerHandshakingInitAction {
                address: action.address,
            });
        }
        Action::PeerConnectionOutgoingError(action) => {
            store.dispatch(PeersGraylistAddressAction {
                address: action.address,
                reason: PeerGraylistReason::ConnectionOutgoingError,
            });
        }
        _ => {}
    }
}
