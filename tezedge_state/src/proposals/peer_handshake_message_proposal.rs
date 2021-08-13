use crate::PeerAddress;
use std::time::Duration;
use tla_sm::Proposal;

pub struct PeerHandshakeMessageProposal<'a, Efs, M> {
    pub effects: &'a mut Efs,
    pub time_passed: Duration,
    pub peer: PeerAddress,
    pub message: M,
}

impl<'a, Efs, M> Proposal for PeerHandshakeMessageProposal<'a, Efs, M> {
    fn time_passed(&self) -> Duration {
        self.time_passed
    }

    fn nullify_time_passed(&mut self) {
        self.time_passed = Duration::new(0, 0);
    }
}
