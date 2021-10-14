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
            // TODO: check peers thresholds.
            state.peers.entry(*address).or_insert_with(|| Peer {
                status: PeerStatus::Connecting(
                    PeerConnectionIncomingState::Pending { token: *token }.into(),
                ),
                quota: PeerQuota {
                    quota_bytes_read: 0,
                    quota_read_timestamp: action.id,
                },
            });
        }
        _ => {}
    }
}
