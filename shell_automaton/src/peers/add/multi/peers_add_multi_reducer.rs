use redux_rs::ActionWithId;

use crate::{State, action::Action, peer::{Peer, PeerQuota, PeerStatus}};

use super::PeersAddMultiAction;

pub fn peers_add_multi_reducer(state: &mut State, action: &ActionWithId<Action>) {
    match &action.action {
        Action::PeersAddMulti(PeersAddMultiAction { addresses }) => {
            // TODO: check peers thresholds.
            for address in addresses {
                state.peers.entry(*address).or_insert_with(|| Peer {
                    status: PeerStatus::Potential,
                    quota: PeerQuota {
                        quota_bytes_read: 0,
                        quota_read_timestamp: action.id,
                    }
                });
            }
        }
        _ => {}
    }
}
