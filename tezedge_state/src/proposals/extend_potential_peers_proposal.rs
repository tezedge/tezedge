use std::time::Instant;
use tla_sm::Proposal;

pub struct ExtendPotentialPeersProposal<'a, Efs, P> {
    pub effects: &'a mut Efs,
    pub at: Instant,
    pub peers: P,
}

impl<'a, Efs, P> Proposal for ExtendPotentialPeersProposal<'a, Efs, P> {
    fn time(&self) -> Instant {
        self.at
    }
}
