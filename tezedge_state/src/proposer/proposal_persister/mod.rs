// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::proposals::RecordedProposal;

mod file_proposal_persister;
pub use file_proposal_persister::*;

mod noop_proposal_persister;
pub use noop_proposal_persister::*;

pub trait ProposalPersister {
    fn persist_proposal<P>(&mut self, proposal: P)
    where
        P: Into<RecordedProposal>;
}
