use crate::proposals::NewPeerConnectProposal;
use crate::{Effects, HandshakeStep, P2pState, PendingPeer, TezedgeState};
use tla_sm::Acceptor;

impl<'a, Efs> Acceptor<NewPeerConnectProposal<'a, Efs>> for TezedgeState
where
    Efs: Effects,
{
    /// Handle new incoming connection.
    fn accept(&mut self, proposal: NewPeerConnectProposal<'a, Efs>) {
        if let Err(_err) = self.validate_proposal(&proposal) {
            #[cfg(test)]
            assert_ne!(_err, crate::InvalidProposalError::ProposalOutdated);
            return;
        }

        if self.is_address_blacklisted(&proposal.peer) {
            slog::debug!(&self.log, "Rejecting incoming peer connection!"; "peer_address" => proposal.peer.to_string(), "reason" => "Peer's IP is blacklisted!");
            self.disconnect_peer(proposal.at, proposal.peer);
        } else {
            match self.p2p_state {
                P2pState::Pending | P2pState::Ready => {
                    self.pending_peers.insert(PendingPeer::new(
                        proposal.peer.clone(),
                        true,
                        HandshakeStep::Initiated { at: proposal.at },
                    ));
                }
                P2pState::PendingFull | P2pState::ReadyFull | P2pState::ReadyMaxed => {
                    slog::debug!(&self.log, "Rejecting incoming peer connection!"; "peer_address" => proposal.peer.to_string(), "reason" => "Max pending/connected peers threshold reached!");
                    self.disconnect_peer(proposal.at, proposal.peer);
                }
            }
        }

        self.adjust_p2p_state(proposal.at, proposal.effects);
        self.periodic_react(proposal.at, proposal.effects);
    }
}
