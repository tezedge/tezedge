use redux_rs::ActionWithId;

use crate::peer::binary_message::read::PeerBinaryMessageReadState;
use crate::peer::{PeerCrypto, PeerHandshaked, PeerStatus};
use crate::{Action, State};

use super::PeerMessageReadState;

pub fn peer_message_read_reducer(state: &mut State, action: &ActionWithId<Action>) {
    match &action.action {
        Action::PeerMessageReadInit(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                match &mut peer.status {
                    PeerStatus::Handshaked(PeerHandshaked {
                        crypto,
                        message_read,
                        ..
                    }) => match message_read {
                        PeerMessageReadState::Success { read_crypto, .. } => {
                            *crypto = PeerCrypto::unsplit_after_reading(
                                read_crypto.clone(),
                                crypto.local_nonce(),
                            );
                            *message_read = PeerMessageReadState::Pending {
                                binary_message_read: PeerBinaryMessageReadState::Init {
                                    crypto: read_crypto.clone(),
                                },
                            };
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
        }
        Action::PeerMessageReadError(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                match &mut peer.status {
                    PeerStatus::Handshaked(PeerHandshaked { message_read, .. }) => {
                        match message_read {
                            PeerMessageReadState::Pending { .. } => {}
                            _ => return,
                        };

                        *message_read = PeerMessageReadState::Error {
                            error: action.error.clone(),
                        };
                    }
                    _ => {}
                }
            }
        }
        Action::PeerMessageReadSuccess(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                match &mut peer.status {
                    PeerStatus::Handshaked(PeerHandshaked { message_read, .. }) => {
                        let read_crypto = match message_read {
                            PeerMessageReadState::Pending {
                                binary_message_read,
                            } => match binary_message_read {
                                PeerBinaryMessageReadState::Ready { crypto, .. } => crypto,
                                _ => return,
                            },
                            _ => return,
                        };

                        *message_read = PeerMessageReadState::Success {
                            read_crypto: read_crypto.clone(),
                            message: action.message.clone(),
                        };
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
}
