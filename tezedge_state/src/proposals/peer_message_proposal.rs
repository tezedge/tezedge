use std::time::Instant;

use tezos_messages::p2p::encoding::peer::PeerMessageResponse;
use tla_sm::Proposal;

use crate::PeerAddress;

#[derive(Debug, Clone)]
pub struct PeerMessageProposal {
    pub at: Instant,
    pub peer: PeerAddress,
    pub message: PeerMessageResponse,
}

impl Proposal for PeerMessageProposal {
    fn time(&self) -> Instant {
        self.at
    }
}
