
use std::convert::TryInto;

use redux_rs::ActionWithId;

use crate::{Action, State};

use super::{PeerTryReadAction, chunk::read::PeerChunkReadPartAction};

pub fn peer_reducer(state: &mut State, action: &ActionWithId<Action>) {
    match &action.action {
        Action::WakeupEvent(_) => {
            let restore_duration = state.config.quota.restore_duration_millis;
            state.peers.iter_mut().for_each(|(address, peer)| {
                let millis = action.id.duration_since(peer.quota.quota_read_timestamp).as_millis();
                if millis.try_into().unwrap_or(usize::MAX) >= restore_duration {
                    eprintln!("[wakeup] resetting read bytes for {}, was {}", address, peer.quota.quota_bytes_read);
                    peer.quota.quota_bytes_read = 0;
                    peer.quota.quota_read_timestamp = action.id;
                }
            });
        }
        Action::PeerTryRead(PeerTryReadAction { address }) => {
            if let Some(peer) = state.peers.get_mut(address) {
                let millis = action.id.duration_since(peer.quota.quota_read_timestamp).as_millis();
                if millis.try_into().unwrap_or(usize::MAX) >= state.config.quota.restore_duration_millis {
                    eprintln!("[tryread] resetting read bytes for {}, was {}", address, peer.quota.quota_bytes_read);
                    peer.quota.quota_bytes_read = 0;
                    peer.quota.quota_read_timestamp = action.id;
                }
            }
        }
        Action::PeerChunkReadPart(PeerChunkReadPartAction { address, bytes }) => {
            if let Some(peer) = state.peers.get_mut(&address) {
                peer.quota.quota_bytes_read += bytes.len();
            }
        }
        _ => (),
    }
}
