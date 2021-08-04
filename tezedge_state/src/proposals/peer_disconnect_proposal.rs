use std::time::Instant;
use tla_sm::Proposal;

use crate::PeerAddress;

/// Disconnect the peer.
pub struct PeerDisconnectProposal<'a, Efs> {
    pub effects: &'a mut Efs,
    pub at: Instant,
    pub peer: PeerAddress,
}

impl<'a, Efs> Proposal for PeerDisconnectProposal<'a, Efs> {
    fn time(&self) -> Instant {
        self.at
    }
}
