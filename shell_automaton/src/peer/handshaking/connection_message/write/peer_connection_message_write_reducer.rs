use redux_rs::ActionWithId;

use crate::peer::handshaking::{MessageWriteState, PeerHandshakingStatus};
use crate::{action::Action, peer::PeerStatus, State};

pub fn peer_connection_message_write_reducer(state: &mut State, action: &ActionWithId<Action>) {
    match &action.action {
        Action::PeerConnectionMessageWriteInit(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                match &mut peer.status {
                    PeerStatus::Handshaking(handshaking) => match handshaking.status {
                        PeerHandshakingStatus::Init => {
                            handshaking.status = PeerHandshakingStatus::ConnectionMessageWrite {
                                conn_msg: action.conn_msg.clone(),
                                status: MessageWriteState::Idle,
                            }
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
        }
        Action::PeerConnectionMessagePartWritten(action) => {
            let conn_msg_write_status = match state.peers.get_mut(&action.address) {
                Some(peer) => match &mut peer.status {
                    PeerStatus::Handshaking(handshaking) => match &mut handshaking.status {
                        PeerHandshakingStatus::ConnectionMessageWrite { status, .. } => status,
                        _ => return,
                    },
                    _ => return,
                },
                None => return,
            };
            // How much checks should there be in reducer? and if some
            // unexpected case is encountered, what do we do in reducer?
            // Should I check here if
            // already_written + action.bytes_written > conn_msg_bytes.len()
            match conn_msg_write_status {
                MessageWriteState::Idle => {
                    *conn_msg_write_status = MessageWriteState::Pending {
                        written: action.bytes_written,
                    };
                }
                MessageWriteState::Pending { written } => {
                    *written += action.bytes_written;
                }
                _ => {}
            }
        }
        Action::PeerConnectionMessageWriteError(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                match &mut peer.status {
                    PeerStatus::Handshaking(handshaking) => match &mut handshaking.status {
                        PeerHandshakingStatus::ConnectionMessageWrite { status, .. } => {
                            *status = MessageWriteState::Error {
                                error: action.error,
                            };
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
        }
        Action::PeerConnectionMessageWriteSuccess(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                match &mut peer.status {
                    PeerStatus::Handshaking(handshaking) => match &mut handshaking.status {
                        PeerHandshakingStatus::ConnectionMessageWrite { status, .. } => {
                            if let MessageWriteState::Pending { .. } = status {
                                *status = MessageWriteState::Success;
                            }
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
        }
        _ => {}
    }
}
