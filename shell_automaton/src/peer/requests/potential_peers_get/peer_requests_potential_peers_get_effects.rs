// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use tezos_messages::p2p::encoding::peer::PeerMessage;

use crate::peer::message::write::PeerMessageWriteInitAction;
use crate::peers::add::multi::PeersAddMultiAction;
use crate::{Action, ActionWithMeta, Service, Store};

use super::{
    PeerRequestsPotentialPeersGetFinishAction, PeerRequestsPotentialPeersGetInitAction,
    PeerRequestsPotentialPeersGetPendingAction,
};

pub fn peer_requests_potential_peers_get_effects<S>(store: &mut Store<S>, action: &ActionWithMeta)
where
    S: Service,
{
    match &action.action {
        Action::PeerRequestsPotentialPeersGetInit(content) => {
            store.dispatch(PeerMessageWriteInitAction {
                address: content.address,
                message: Arc::new(PeerMessage::Bootstrap.into()),
            });
            store.dispatch(PeerRequestsPotentialPeersGetPendingAction {
                address: content.address,
            });
        }
        Action::MioTimeoutEvent(_)
        | Action::PeerDisconnected(_)
        | Action::PeerRequestsPotentialPeersGetError(_) => {
            request_potential_peers_from_any_peer(store);
        }
        Action::PeerRequestsPotentialPeersGetSuccess(content) => {
            store.dispatch(PeersAddMultiAction {
                addresses: content.result.clone(),
            });
            store.dispatch(PeerRequestsPotentialPeersGetFinishAction {
                address: content.address,
            });
            request_potential_peers_from_any_peer(store);
        }
        Action::PeerHandshakingFinish(content) => {
            let address = content.address;
            store.dispatch(PeerRequestsPotentialPeersGetInitAction { address });
        }
        _ => {}
    }
}

pub fn request_potential_peers_from_any_peer<S>(store: &mut Store<S>)
where
    S: Service,
{
    let state = store.state.get();
    if !PeerRequestsPotentialPeersGetInitAction::should_request(state) {
        return;
    }

    let address = match state.peers.handshaked_iter().find(|(address, _)| {
        PeerRequestsPotentialPeersGetInitAction::can_request_from_peer(state, *address)
    }) {
        Some((address, _)) => address,
        None => return,
    };
    store.dispatch(PeerRequestsPotentialPeersGetInitAction { address });
}
