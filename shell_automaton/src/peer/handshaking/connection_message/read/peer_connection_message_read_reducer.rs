use redux_rs::ActionWithId;

use crate::tmp::chunking::ChunkReadBuffer;

use crate::{action::Action, peer::PeerStatus, State};

use crate::peer::handshaking::{
    ConnectionMessageDataReceived, MessageReadState, PeerHandshakingStatus,
};

pub fn peer_connection_message_read_reducer(state: &mut State, action: &ActionWithId<Action>) {
    match &action.action {
        Action::PeerConnectionMessageReadInit(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                match &mut peer.status {
                    PeerStatus::Handshaking(handshaking) => match &handshaking.status {
                        PeerHandshakingStatus::ConnectionMessageWrite { conn_msg, status } => {
                            handshaking.status = PeerHandshakingStatus::ConnectionMessageRead {
                                // TODO: cloning could be avoiding by
                                // mem::replace with `Init`.
                                conn_msg_written: conn_msg.clone(),
                                status: MessageReadState::Idle,
                            };
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
        }
        Action::PeerConnectionMessagePartRead(action) => {
            let conn_msg_read_status = match state.peers.get_mut(&action.address) {
                Some(peer) => match &mut peer.status {
                    PeerStatus::Handshaking(handshaking) => match &mut handshaking.status {
                        PeerHandshakingStatus::ConnectionMessageRead { status, .. } => status,
                        _ => return,
                    },
                    _ => return,
                },
                None => return,
            };
            match conn_msg_read_status {
                MessageReadState::Idle => {
                    let mut buffer = ChunkReadBuffer::new();
                    buffer.read_from(&mut &action.bytes[..]);
                    *conn_msg_read_status = MessageReadState::Pending { buffer };
                }
                MessageReadState::Pending { buffer } => {
                    buffer.read_from(&mut &action.bytes[..]);
                }
                _ => {}
            }
        }
        Action::PeerConnectionMessageReadError(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                match &mut peer.status {
                    PeerStatus::Handshaking(handshaking) => match &mut handshaking.status {
                        PeerHandshakingStatus::ConnectionMessageRead { status, .. } => {
                            *status = MessageReadState::Error {
                                error: action.error,
                            };
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
        }
        Action::PeerConnectionMessageReadSuccess(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                let status = match &mut peer.status {
                    PeerStatus::Handshaking(handshaking) => {
                        if let Err(nack_motive) = action.compatible_version.as_ref() {
                            // TODO: set nack motive
                        }
                        match &mut handshaking.status {
                            PeerHandshakingStatus::ConnectionMessageRead { status, .. } => status,
                            _ => return,
                        }
                    }
                    _ => return,
                };

                if let MessageReadState::Pending { buffer } = status {
                    if let Some(encoded) = buffer.take_if_ready() {
                        *status = MessageReadState::Success {
                            message: ConnectionMessageDataReceived {
                                port: action.port,
                                compatible_version: action.compatible_version.clone().ok(),
                                public_key: action.public_key.clone(),
                                encoded,
                            },
                        };
                    }
                }
            }
        }
        _ => {}
    }
}
