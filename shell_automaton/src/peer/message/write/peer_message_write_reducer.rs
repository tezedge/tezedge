// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::peer::binary_message::write::PeerBinaryMessageWriteState;
use crate::peer::{PeerCrypto, PeerHandshaked, PeerStatus};
use crate::{Action, ActionWithMeta, State};

pub fn peer_message_write_reducer(state: &mut State, action: &ActionWithMeta) {
    match &action.action {
        Action::PeerMessageWriteInit(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                match &mut peer.status {
                    PeerStatus::Handshaked(PeerHandshaked { message_write, .. }) => {
                        message_write.queue.push_back(action.message.clone());
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
                            message_write.queue.pop_front();
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
