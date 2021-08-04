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
            #[cfg(test)]
            assert_ne!(_err, crate::InvalidProposalError::ProposalOutdated);
            return;
        }
        self.extend_potential_peers(proposal.peers.into_iter().map(|x| x.into()));
        self.initiate_handshakes(proposal.at, proposal.effects);
        self.periodic_react(proposal.at, proposal.effects);
    }
}
