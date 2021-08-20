// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use super::ProposalPersister;

/// Does nothing with passed proposals for persisting. Simply discards them.
#[derive(Debug, Clone)]
pub struct NoopProposalPersister;

impl ProposalPersister for NoopProposalPersister {
    fn persist_proposal<P>(&mut self, _: P)
    where
        P: Into<crate::proposals::RecordedProposal>,
    {
    }
}
