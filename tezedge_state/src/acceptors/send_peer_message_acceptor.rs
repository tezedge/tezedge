// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use tla_sm::Acceptor;

use crate::proposals::SendPeerMessageProposal;
use crate::{Effects, TezedgeState};

impl<'a, Efs> Acceptor<SendPeerMessageProposal<'a, Efs>> for TezedgeState
where
    Efs: Effects,
{
    /// Handle request by Proposer to send a message to the peer.
    fn accept(&mut self, proposal: SendPeerMessageProposal<'a, Efs>) {
        if let Err(_err) = self.validate_proposal(&proposal) {
            return;
        }

        if let Some(peer) = self.connected_peers.get_mut(&proposal.peer) {
            peer.enqueue_send_message(proposal.message);
        } else {
            slog::debug!(&self.log, "Disconnecting peer!"; "reason" => "Received request from Proposer to send a message for non-existant peer.");
            self.disconnect_peer(proposal.peer);
        }

        self.adjust_p2p_state(proposal.effects);
        self.periodic_react(proposal.effects);
    }
}
