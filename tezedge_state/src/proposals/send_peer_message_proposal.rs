use std::fmt::{self, Debug};
use std::time::Instant;
use tezos_messages::p2p::encoding::peer::PeerMessage;
use tla_sm::Proposal;

use crate::PeerAddress;

pub struct SendPeerMessageProposal<'a, Efs> {
    pub effects: &'a mut Efs,
    pub at: Instant,
    pub peer: PeerAddress,
    pub message: PeerMessage,
}

impl<'a, Efs> Debug for SendPeerMessageProposal<'a, Efs> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PeerSendMessageProposal")
            .field("at", &self.at)
            .field("peer", &self.peer)
            .finish()
    }
}

impl<'a, Efs> Proposal for SendPeerMessageProposal<'a, Efs> {
    fn time(&self) -> Instant {
        self.at
    }
}
