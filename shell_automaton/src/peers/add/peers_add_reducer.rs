use redux_rs::ActionWithId;

use crate::{
    action::Action,
    peer::{connection::incoming::PeerConnectionIncomingState, Peer, PeerStatus},
    State,
};

use super::PeersAddIncomingPeerAction;

pub fn peers_add_reducer(state: &mut State, action: &ActionWithId<Action>) {
    match &action.action {
        Action::PeersAddIncomingPeer(PeersAddIncomingPeerAction { address, token }) => {
            state.peers.entry(*address).or_insert_with(|| {
                Peer::new(
                    PeerStatus::Connecting(
                        PeerConnectionIncomingState::Pending {
                            time: action.time_as_nanos(),
                            token: *token,
                        }
                        .into(),
                    ),
                    action.id,
                )
            });
        }
        _ => {}
    }
}
