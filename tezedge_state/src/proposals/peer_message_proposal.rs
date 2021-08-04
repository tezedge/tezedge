use std::time::Instant;

use tezos_messages::p2p::encoding::peer::PeerMessageResponse;
use tla_sm::Proposal;

use crate::PeerAddress;

pub struct PeerMessageProposal<'a, Efs> {
    pub effects: &'a mut Efs,
    pub at: Instant,
    pub peer: PeerAddress,
    pub message: PeerMessageResponse,
}

impl<'a, Efs> Proposal for PeerMessageProposal<'a, Efs> {
    fn time(&self) -> Instant {
        self.at
    }
}
