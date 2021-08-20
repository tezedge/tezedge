// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::net::SocketAddr;

use crate::proposals::ExtendPotentialPeersProposal;
use crate::{Effects, TezedgeState};
use tla_sm::Acceptor;

impl<'a, Efs, P> Acceptor<ExtendPotentialPeersProposal<'a, Efs, P>> for TezedgeState
where
    Efs: Effects,
    P: IntoIterator<Item = SocketAddr>,
{
    fn accept(&mut self, proposal: ExtendPotentialPeersProposal<'a, Efs, P>) {
        if let Err(_err) = self.validate_proposal(&proposal) {
            return;
        }
        self.extend_potential_peers(proposal.peers.into_iter().map(|x| x.into()));
        self.initiate_handshakes(proposal.effects);
        self.periodic_react(proposal.effects);
    }
}
