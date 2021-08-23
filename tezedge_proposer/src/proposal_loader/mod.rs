// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use tezedge_state::proposals::RecordedProposal;

mod file_proposal_loader;
pub use file_proposal_loader::*;

mod noop_proposal_loader;
pub use noop_proposal_loader::*;

pub trait ProposalLoader: Iterator<Item = Result<RecordedProposal, failure::Error>> {}

impl<T> ProposalLoader for T where T: Iterator<Item = Result<RecordedProposal, failure::Error>> {}
