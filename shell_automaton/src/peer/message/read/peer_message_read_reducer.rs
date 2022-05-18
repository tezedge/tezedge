// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use slog::FnValue;
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
            if let PeerMessageReadState::Success {
                read_crypto,
                message,
                ..
            } = &mut peer.message_read
            {
                peer.crypto = PeerCrypto::unsplit_after_reading(
                    read_crypto.clone(),
                    peer.crypto.local_nonce(),
                );

                if let PeerMessage::BlockHeader(m) = message.message() {
                    let pending_requests = &mut state.peers.pending_block_header_requests;
                    let _ = m
                        .block_header()
                        .message_typed_hash()
                        .map(|b| pending_requests.remove(&b));
                };

                peer.message_read = PeerMessageReadState::Pending {
                    binary_message_read: PeerBinaryMessageReadState::Init {
                        crypto: read_crypto.clone(),
                    },
                };
            }
        }
        Action::PeerMessageReadError(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                if let PeerStatus::Handshaked(PeerHandshaked { message_read, .. }) =
                    &mut peer.status
                {
                    match message_read {
                        PeerMessageReadState::Pending { .. } => {}
                        _ => return,
                    };

                    *message_read = PeerMessageReadState::Error {
                        error: action.error.clone(),
                    };
                }
            }
        }
        Action::PeerMessageReadSuccess(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                if let PeerStatus::Handshaked(PeerHandshaked { message_read, .. }) =
                    &mut peer.status
                {
                    let read_crypto = match message_read {
                        PeerMessageReadState::Pending {
                            binary_message_read: PeerBinaryMessageReadState::Ready { crypto, .. },
                        } => crypto,
                        _ => return,
                    };

                    slog::trace!(state.log, "Peer message read success";
                                 "address" => FnValue(|_| format!("{}", action.address)),
                                 "message" => FnValue(|_| format!("{:?}", action.message))
                    );

                    *message_read = PeerMessageReadState::Success {
                        read_crypto: read_crypto.clone(),
                        message: action.message.clone(),
                    };
                }
            }
        }
        Action::PeerCurrentHeadUpdate(content) => {
            if let Some(peer) = state.peers.get_handshaked_mut(&content.address) {
                peer.current_head = Some(content.current_head.clone());
                peer.current_head_last_update = Some(action.time_as_nanos());
            }
        }
        _ => {}
    }
}
