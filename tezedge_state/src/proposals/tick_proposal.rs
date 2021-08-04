use std::time::Instant;

use crate::Proposal;

/// `TickProposal` is a way for us to update logical clock for state machine.
///
/// Every `Proposal` updates logical clock of the state machine after
/// it has been fed to `Acceptor`. This is in case we want to explicitly
/// update time, or if we haven't sent proposals to state machine for
/// some time and want to update time.
pub struct TickProposal<'a, Efs> {
    pub effects: &'a mut Efs,
    pub at: Instant,
}

impl<'a, Efs> Proposal for TickProposal<'a, Efs> {
    fn time(&self) -> Instant {
        self.at
    }
}
