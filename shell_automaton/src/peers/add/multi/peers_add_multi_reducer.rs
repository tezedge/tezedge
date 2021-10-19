use redux_rs::ActionWithId;

use crate::{
    action::Action,
    peer::{Peer, PeerStatus},
    State,
};

use super::PeersAddMultiAction;

pub fn peers_add_multi_reducer(state: &mut State, action: &ActionWithId<Action>) {
    match &action.action {
        Action::PeersAddMulti(PeersAddMultiAction { addresses }) => {
            let max_len = state
                .config
                .peers_potential_max
                .checked_sub(state.peers.potential_len())
                .unwrap_or(0);

            for address in addresses.into_iter().take(max_len) {
                state
                    .peers
                    .entry(*address)
                    .or_insert_with(|| Peer::new(PeerStatus::Potential, action.id));
            }
        }
        _ => {}
    }
}
