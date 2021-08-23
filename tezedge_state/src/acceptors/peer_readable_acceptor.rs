// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::io::{self, Read, Write};

use crate::chunking::ReadMessageError;
use crate::proposals::peer_handshake_message::PeerBinaryHandshakeMessage;
use crate::proposals::{
    PeerHandshakeMessageProposal, PeerMessageProposal, PeerReadableProposal, PeerWritableProposal,
};
use crate::{Effects, TezedgeState};
use tla_sm::Acceptor;

impl<'a, Efs, S> Acceptor<PeerReadableProposal<'a, Efs, S>> for TezedgeState
where
    Efs: Effects,
    S: Read + Write,
{
    /// Peer's stream might be ready for reading, try to read from passed stream.
    fn accept(&mut self, proposal: PeerReadableProposal<'a, Efs, S>) {
        if let Some(peer) = self.connected_peers.get_mut(&proposal.peer) {
            match peer.read_message_from(proposal.stream) {
                Ok(message) => {
                    self.accept_internal(PeerMessageProposal {
                        effects: proposal.effects,
                        peer: proposal.peer,
                        message,
                    });
                    self.accept_internal(proposal);
                    return;
                }
                Err(ReadMessageError::Pending) => {}
                Err(ReadMessageError::QuotaReached) => {}
                Err(err) => {
                    slog::warn!(&self.log, "Read failed!"; "description" => "error while trying to read from connected peer stream.", "error" => format!("{:?}", err));
                    self.blacklist_peer(proposal.peer);
                }
            };
        } else {
            if let Some(peer) = self.pending_peers.get_mut(&proposal.peer) {
                // we won't try to read message from the peer unless
                // we are waiting for a message.
                if !peer.should_read() {
                    return;
                }
                if let Err(err) = peer.read_message_from(proposal.stream) {
                    match err.kind() {
                        io::ErrorKind::WouldBlock => {}
                        _ => {
                            slog::warn!(&self.log, "Read failed!"; "description" => "error while trying to read from peer stream during handshake.", "error" => format!("{:?}", err));
                            self.blacklist_peer(proposal.peer);
                        }
                    }
                } else if let Some(message) = peer.read_buf.take_if_ready() {
                    self.accept_internal(PeerHandshakeMessageProposal {
                        effects: proposal.effects,
                        peer: proposal.peer,
                        message: PeerBinaryHandshakeMessage::new(message),
                    });
                    // try writing to peer after succesfully reading a message.
                    return self.accept_internal(PeerWritableProposal::from(proposal));
                }
            } else {
                // we received event for a non existant peer. Can happen
                // and its normal, unless mio is out of sync.
                self.disconnect_peer(proposal.peer);
            }
        }

        self.adjust_p2p_state(proposal.effects);
    }
}
