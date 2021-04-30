use redux_rs::ActionWithId;

use crate::{
    action::Action,
    peer::{connection::incoming::PeerConnectionIncomingState, Peer, PeerStatus},
    State,
};

pub fn peers_add_reducer(state: &mut State, action: &ActionWithId<Action>) {
    match &action.action {
        Action::PeersAddIncomingPeer(action) => {
            // TODO: check peers thresholds.
            state.peers.entry(action.address).or_insert_with(|| Peer {
                status: PeerStatus::Connecting(
                    PeerConnectionIncomingState::Pending {
                        token: action.token,
                    }
                    .into(),
                ),
            });
        }
        _ => {}
    }
}
