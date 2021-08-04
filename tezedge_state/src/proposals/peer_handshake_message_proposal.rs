use crate::PeerAddress;
use std::time::Instant;
use tla_sm::Proposal;

pub struct PeerHandshakeMessageProposal<'a, Efs, M> {
    pub effects: &'a mut Efs,
    pub at: Instant,
    pub peer: PeerAddress,
    pub message: M,
}

impl<'a, Efs, M> Proposal for PeerHandshakeMessageProposal<'a, Efs, M> {
    fn time(&self) -> Instant {
        self.at
    }
}
