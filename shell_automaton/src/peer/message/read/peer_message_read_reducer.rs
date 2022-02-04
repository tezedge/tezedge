// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use tezos_messages::p2p::binary_message::MessageHash;
use tezos_messages::p2p::encoding::peer::PeerMessage;

use crate::peer::binary_message::read::PeerBinaryMessageReadState;
use crate::peer::{PeerCrypto, PeerHandshaked, PeerStatus};
use crate::{Action, ActionWithMeta, State};

use super::PeerMessageReadState;

pub fn peer_message_read_reducer(state: &mut State, action: &ActionWithMeta) {
    match &action.action {
        Action::PeerMessageReadInit(content) => {
            let peer = match state
                .peers
                .list
                .get_mut(&content.address)
                .and_then(|v| v.status.as_handshaked_mut())
            {
                Some(v) => v,
                None => return,
            };
            match &mut peer.message_read {
                PeerMessageReadState::Success {
                    read_crypto,
                    message,
                    ..
                } => {
                    peer.crypto = PeerCrypto::unsplit_after_reading(
                        read_crypto.clone(),
                        peer.crypto.local_nonce(),
                    );

                    match message.message() {
                        PeerMessage::BlockHeader(m) => {
                            let pending_requests = &mut state.peers.pending_block_header_requests;
                            let _ = m
                                .block_header()
                                .message_typed_hash()
                                .map(|b| pending_requests.remove(&b));
                        }
                        _ => {}
                    };

                    peer.message_read = PeerMessageReadState::Pending {
                        binary_message_read: PeerBinaryMessageReadState::Init {
                            crypto: read_crypto.clone(),
                        },
                    };
                }
                _ => {}
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
                    PeerStatus::Handshaked(PeerHandshaked {
                        message_read,
                        current_head_level,
                        ..
                    }) => {
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

                        // TODO(zura): Maybe move to separate actions?
                        match action.message.message() {
                            PeerMessage::CurrentHead(msg) => {
                                let level = msg.current_block_header().level();
                                // update known current head for peer.
                                *current_head_level = current_head_level
                                    .map(|old_level| level.max(old_level))
                                    .or_else(|| Some(level));
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
}
