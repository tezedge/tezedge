use std::time::Instant;
use tla_sm::Proposal;

use crate::PeerAddress;

pub struct PeerBlacklistProposal<'a, Efs> {
    pub effects: &'a mut Efs,
    pub at: Instant,
    pub peer: PeerAddress,
}

impl<'a, Efs> Proposal for PeerBlacklistProposal<'a, Efs> {
    fn time(&self) -> Instant {
        self.at
    }
}
