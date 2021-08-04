use std::fmt::{self, Debug};
use std::time::Instant;
use tla_sm::Proposal;

use crate::PeerAddress;

use super::PeerReadableProposal;

// TODO: there should be difference between 2 kinds of proposals. 1 is
// normal one, which is meant to be recorded and if replayed will give
// us exact same state. and 2nd is like this one, which is purely just
// for performance reasons. Since we invoke read on reader which makes a
// syscall to read certain bytes means that those bytes aren't available
// and are consumed on read, hence hard to debug. So these types of proposals
// should simply invoke another proposal like in this case that the chunk
// is ready so that proposal can be recorded and replayed.
pub struct PeerWritableProposal<'a, Efs, S> {
    pub effects: &'a mut Efs,
    pub at: Instant,
    pub peer: PeerAddress,
    pub stream: &'a mut S,
}

impl<'a, Efs, S> Debug for PeerWritableProposal<'a, Efs, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PeerWritableProposal")
            .field("at", &self.at)
            .field("peer", &self.peer)
            .finish()
    }
}

impl<'a, Efs, S> Proposal for PeerWritableProposal<'a, Efs, S> {
    fn time(&self) -> Instant {
        self.at
    }
}

impl<'a, Efs, S> From<PeerReadableProposal<'a, Efs, S>> for PeerWritableProposal<'a, Efs, S> {
    fn from(proposal: PeerReadableProposal<'a, Efs, S>) -> Self {
        Self {
            effects: proposal.effects,
            at: proposal.at,
            peer: proposal.peer,
            stream: proposal.stream,
        }
    }
}
