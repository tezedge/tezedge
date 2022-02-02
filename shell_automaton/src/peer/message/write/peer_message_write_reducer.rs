// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use tezos_messages::p2p::encoding::peer::PeerMessage;

use crate::peer::binary_message::write::PeerBinaryMessageWriteState;
use crate::peer::{PeerCrypto, PeerHandshaked, PeerStatus};
use crate::{Action, ActionWithMeta, State};

pub fn peer_message_write_reducer(state: &mut State, action: &ActionWithMeta) {
    match &action.action {
        Action::PeerMessageWriteInit(content) => {
            let peer = match state
                .peers
                .get_mut(&content.address)
                .and_then(|v| v.status.as_handshaked_mut())
            {
                Some(v) => v,
                None => return,
            };
            peer.message_write.queue.push_back(content.message.clone());

            if !matches!(
                &peer.message_write.current,
                PeerBinaryMessageWriteState::Init { .. }
            ) {
                return;
            }
            match content.message.message() {
                PeerMessage::GetBlockHeaders(m) => {
                    m.get_block_headers().iter().for_each(|b| {
                        state
                            .peers
                            .pending_block_header_requests
                            .insert(b.clone(), action.time_as_nanos());
                    });
                }
                _ => {}
            }
        }
        Action::PeerMessageWriteSuccess(content) => {
            if let Some(peer) = state.peers.get_mut(&content.address) {
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
