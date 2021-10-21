use redux_rs::ActionWithId;

use crate::{
    event::P2pPeerEvent,
    peer::{Peer, PeerStatus},
    Action, State,
};

use super::{
    PeerErrorAction, PeerReadState, PeerReadWouldBlockAction, PeerTryReadAction,
    PeerTryWriteAction, PeerWriteState, PeerWriteWouldBlockAction,
};

pub fn peer_reducer(state: &mut State, action: &ActionWithId<Action>) {
    match &action.action {
        Action::P2pPeerEvent(P2pPeerEvent {
            address,
            is_closed,
            is_readable,
            is_writable,
            ..
        }) => {
            if let Some(peer) = state.peers.get_mut(address) {
                match peer.read_state {
                    PeerReadState::Closed => (),
                    _ if *is_closed => peer.read_state = PeerReadState::Closed,
                    PeerReadState::Idle {
                        bytes_read,
                        timestamp,
                    } if *is_readable => {
                        peer.read_state = PeerReadState::Readable {
                            bytes_read,
                            timestamp,
                        }
                    }
                    _ => (),
                }
                match peer.write_state {
                    PeerWriteState::Closed => (),
                    _ if *is_closed => peer.write_state = PeerWriteState::Closed,
                    PeerWriteState::Idle {
                        bytes_written,
                        timestamp,
                    } if *is_writable => {
                        peer.write_state = PeerWriteState::Writable {
                            bytes_written,
                            timestamp,
                        }
                    }
                    _ => (),
                }
            }
        }
        Action::PeerTryRead(PeerTryReadAction { address }) => {
            let restore_duration_millis = state.config.quota.restore_duration_millis;
            if let Some(peer) = state.peers.get_mut(address) {
                if peer
                    .read_state
                    .time_since_last_update(&action.id)
                    .map(|time| time >= restore_duration_millis)
                    .unwrap_or(false)
                {
                    match &peer.read_state {
                        PeerReadState::OutOfQuota { .. } | PeerReadState::Readable { .. } => {
                            peer.read_state = PeerReadState::Readable {
                                bytes_read: 0,
                                timestamp: action.id,
                            };
                        }
                        _ => (),
                    }
                }
            }
        }
        Action::PeerTryWrite(PeerTryWriteAction { address }) => {
            let restore_duration_millis = state.config.quota.restore_duration_millis;
            if let Some(peer) = state.peers.get_mut(address) {
                if peer
                    .write_state
                    .time_since_last_update(&action.id)
                    .map(|time| time >= restore_duration_millis)
                    .unwrap_or(false)
                {
                    match &peer.write_state {
                        PeerWriteState::OutOfQuota { .. } | PeerWriteState::Writable { .. } => {
                            peer.write_state = PeerWriteState::Writable {
                                bytes_written: 0,
                                timestamp: action.id,
                            };
                        }
                        _ => (),
                    }
                }
            }
        }
        Action::PeerReadWouldBlock(PeerReadWouldBlockAction { address }) => {
            if let Some(peer) = state.peers.get_mut(address) {
                if let PeerReadState::Readable {
                    bytes_read,
                    timestamp,
                } = peer.read_state
                {
                    peer.read_state = PeerReadState::Idle {
                        bytes_read,
                        timestamp,
                    };
                }
            }
        }
        Action::PeerWriteWouldBlock(PeerWriteWouldBlockAction { address }) => {
            if let Some(peer) = state.peers.get_mut(address) {
                if let PeerWriteState::Writable {
                    bytes_written,
                    timestamp,
                } = peer.write_state
                {
                    peer.write_state = PeerWriteState::Idle {
                        bytes_written,
                        timestamp,
                    };
                }
            }
        }
        Action::PeerError(PeerErrorAction { address, error }) => {
            match state.peers.get_mut(address) {
                Some(Peer {
                    status: PeerStatus::Error(_),
                    ..
                }) => (),
                Some(Peer { status, .. }) => {
                    *status = PeerStatus::Error(error.clone());
                }
                _ => (),
            }
        }
        _ => (),
    }
}
