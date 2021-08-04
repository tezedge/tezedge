use std::time::Instant;
use tla_sm::Proposal;

use crate::PeerAddress;

/// Peer has disconnected.
pub struct PeerDisconnectedProposal<'a, Efs> {
    pub effects: &'a mut Efs,
    pub at: Instant,
    pub peer: PeerAddress,
}

impl<'a, Efs> Proposal for PeerDisconnectedProposal<'a, Efs> {
    fn time(&self) -> Instant {
        self.at
    }
}
