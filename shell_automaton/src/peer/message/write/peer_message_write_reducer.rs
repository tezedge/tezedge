use std::sync::Arc;

use redux_rs::ActionWithId;

use crate::peer::binary_message::write::PeerBinaryMessageWriteState;
use crate::peer::{PeerCrypto, PeerHandshaked, PeerStatus};
use crate::{Action, State};

pub fn peer_message_write_reducer(state: &mut State, action: &ActionWithId<Action>) {
    match &action.action {
        Action::PeerMessageWriteInit(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                match &mut peer.status {
                    PeerStatus::Handshaked(PeerHandshaked { message_write, .. }) => {
                        // check the current message against the head of the queue
                        // to see if the current one is the the next queued one
                        if let Some(front_msg) = message_write.queue.front() {
                            // pop message from the queue, if that's the
                            // message that we are initiating write for.
                            if Arc::ptr_eq(front_msg, &action.message) {
                                message_write.queue.pop_front();
                            }
                        }


                        if let PeerBinaryMessageWriteState::Init { .. } = &message_write.current {
                            // if binary message writing is idle, immediately start writing the message
                        } else {
                            // otherwise queue it for later reading
                            message_write.queue.push_back(action.message.clone());
                        }
                    }
                    _ => {}
                }
            }
        }
        Action::PeerMessageWriteSuccess(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                match &mut peer.status {
                    PeerStatus::Handshaked(PeerHandshaked {
                        crypto,
                        message_write,
                        ..
                    }) => match &message_write.current {
                        PeerBinaryMessageWriteState::Ready {
                            crypto: write_crypto,
                        } => {
                            let write_crypto = write_crypto.clone();
                            *crypto = PeerCrypto::unsplit_after_writing(
                                write_crypto.clone(),
                                crypto.remote_nonce(),
                            );
                            message_write.current = PeerBinaryMessageWriteState::Init {
                                crypto: write_crypto,
                            };
                        }
                        _ => return,
                    },
                    _ => {}
                }
            }
        }
        _ => {}
    }
}
