use redux_rs::ActionWithId;

use crate::{action::Action, peer::PeerStatus, State};

pub fn peers_remove_reducer(state: &mut State, action: &ActionWithId<Action>) {
    match &action.action {
        Action::PeersRemove(action) => {
            if let Some(peer) = state.peers.get(&action.address) {
                // we aren't allowed to remove peer until peer is disconnected.
                if matches!(&peer.status, PeerStatus::Disconnected) {
                    state.peers.remove(&action.address);
                }
            }
        }
        _ => {}
    }
}
