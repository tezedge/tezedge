// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::proposals::PeerBlacklistProposal;
use crate::{Effects, TezedgeState};
use tla_sm::Acceptor;

impl<'a, Efs> Acceptor<PeerBlacklistProposal<'a, Efs>> for TezedgeState
where
    Efs: Effects,
{
    fn accept(&mut self, proposal: PeerBlacklistProposal<'a, Efs>) {
        slog::warn!(&self.log, "Blacklisting peer"; "peer_address" => proposal.peer.to_string(), "reason" => "Requested by the Proposer");
        self.blacklist_peer(proposal.peer);
        self.adjust_p2p_state(proposal.effects);
    }
}
