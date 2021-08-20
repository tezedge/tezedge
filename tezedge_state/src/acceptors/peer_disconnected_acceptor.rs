// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::proposals::PeerDisconnectedProposal;
use crate::{Effects, TezedgeState};
use tla_sm::Acceptor;

impl<'a, Efs> Acceptor<PeerDisconnectedProposal<'a, Efs>> for TezedgeState
where
    Efs: Effects,
{
    fn accept(&mut self, proposal: PeerDisconnectedProposal<'a, Efs>) {
        if let Err(_err) = self.validate_proposal(&proposal) {
            return;
        }

        slog::warn!(&self.log, "Blacklisting peer"; "reason" => "peer disconnected", "peer_address" => proposal.peer.to_string());
        self.blacklist_peer(proposal.peer);

        self.periodic_react(proposal.effects);
    }
}
