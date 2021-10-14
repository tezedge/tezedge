use redux_rs::ActionWithId;

use crate::{Action, State};

use super::{
    chunk::{read::PeerChunkReadPartAction, write::PeerChunkWritePartAction},
    PeerTryReadAction, PeerTryWriteAction,
};

pub fn peer_reducer(state: &mut State, action: &ActionWithId<Action>) {
    match &action.action {
        Action::PeerTryRead(PeerTryReadAction { address }) => {
            if let Some(peer) = state.peers.get_mut(address) {
                if peer.quota.bytes_read == 0 {
                    return;
                }
                let duration_since_restore_millis = action
                    .id
                    .duration_since(peer.quota.read_timestamp)
                    .as_millis();
                if duration_since_restore_millis >= state.config.quota.restore_duration_millis {
                    peer.quota.bytes_read = 0;
                    peer.quota.read_timestamp = action.id;
                    peer.quota.reject_read = false;
                }
            }
        }
        Action::PeerTryWrite(PeerTryWriteAction { address }) => {
            if let Some(peer) = state.peers.get_mut(address) {
                if peer.quota.bytes_written == 0 {
                    return;
                }
                let duration_since_restore_millis = action
                    .id
                    .duration_since(peer.quota.write_timestamp)
                    .as_millis();
                if duration_since_restore_millis >= state.config.quota.restore_duration_millis {
                    peer.quota.bytes_written = 0;
                    peer.quota.write_timestamp = action.id;
                    peer.quota.reject_write = false;
                }
            }
        }
        Action::PeerChunkReadPart(PeerChunkReadPartAction { address, bytes }) => {
            if let Some(peer) = state.peers.get_mut(&address) {
                peer.quota.bytes_read += bytes.len();
                peer.quota.reject_read = peer.quota.bytes_read >= state.config.quota.read_quota;
            }
        }
        Action::PeerChunkWritePart(PeerChunkWritePartAction { address, written }) => {
            if let Some(peer) = state.peers.get_mut(&address) {
                peer.quota.bytes_written += written;
                peer.quota.reject_write =
                    peer.quota.bytes_written >= state.config.quota.write_quota;
            }
        }
        _ => (),
    }
}
