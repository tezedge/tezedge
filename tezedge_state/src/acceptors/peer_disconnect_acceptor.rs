use crate::proposals::PeerDisconnectProposal;
use crate::{Effects, TezedgeState};
use tla_sm::Acceptor;

impl<'a, Efs> Acceptor<PeerDisconnectProposal<'a, Efs>> for TezedgeState
where
    Efs: Effects,
{
    fn accept(&mut self, proposal: PeerDisconnectProposal<'a, Efs>) {
        if let Err(_err) = self.validate_proposal(&proposal) {
            return;
        }

        slog::warn!(&self.log, "Disconnecting peer"; "peer_address" => proposal.peer.to_string(), "reason" => "Requested by the Proposer");
        self.disconnect_peer(proposal.peer);

        self.periodic_react(proposal.effects);
    }
}
