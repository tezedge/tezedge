use redux_rs::ActionWithId;

use crate::{
    action::Action,
    peer::{connection::incoming::PeerConnectionIncomingState, Peer, PeerQuota, PeerStatus},
    State,
};

use super::PeersAddIncomingPeerAction;

pub fn peers_add_reducer(state: &mut State, action: &ActionWithId<Action>) {
    match &action.action {
        Action::PeersAddIncomingPeer(PeersAddIncomingPeerAction { address, token }) => {
            if let Ok(entry) = state.peers.entry(*address) {
                entry.or_insert_with(|| Peer {
                    status: PeerStatus::Connecting(
                        PeerConnectionIncomingState::Pending {
                            time: action.time_as_nanos(),
                            token: *token,
                        }
                        .into(),
                    ),
                    quota: PeerQuota::new(action.id),
                });
            }
        }
        _ => {}
    }
}
