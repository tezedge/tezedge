// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::proposals::RecordedProposal;

/// Empty proposal loader.
#[derive(Debug, Clone)]
pub struct NoopProposalLoader;

impl Iterator for NoopProposalLoader {
    type Item = Result<RecordedProposal, failure::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        None
    }
}
