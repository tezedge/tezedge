use redux_rs::ActionWithId;

use crate::{
    action::Action,
    peer::{Peer, PeerStatus},
    State,
};

pub fn peers_add_multi_reducer(state: &mut State, action: &ActionWithId<Action>) {
    match &action.action {
        Action::PeersAddMulti(action) => {
            // TODO: check peers thresholds.
            for address in &action.addresses {
                state.peers.entry(*address).or_insert_with(|| Peer {
                    status: PeerStatus::Potential,
                });
            }
        }
        _ => {}
    }
}
