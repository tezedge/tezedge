// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::time::Duration;
use tla_sm::recorders::CloneRecorder;
use tla_sm::DefaultRecorder;

use crate::{EffectsRecorder, Proposal, RecordedEffects};

use super::MaybeRecordedProposal;

/// `TickProposal` is a way for us to update logical clock for state machine.
///
/// Every `Proposal` updates logical clock of the state machine after
/// it has been fed to `Acceptor`. This is in case we want to explicitly
/// update time, or if we haven't sent proposals to state machine for
/// some time and want to update time.
pub struct TickProposal<'a, Efs> {
    pub effects: &'a mut Efs,
    pub time_passed: Duration,
}

impl<'a, Efs> Proposal for TickProposal<'a, Efs> {
    fn time_passed(&self) -> Duration {
        self.time_passed
    }

    fn nullify_time_passed(&mut self) {
        self.time_passed = Duration::new(0, 0);
    }
}

impl<'a, Efs> TickProposal<'a, Efs> {
    pub fn default_recorder(self) -> TickProposalRecorder<'a, Efs> {
        TickProposalRecorder::new(self)
    }
}

impl<'a, Efs> DefaultRecorder for TickProposal<'a, Efs> {
    type Recorder = TickProposalRecorder<'a, Efs>;

    fn default_recorder(self) -> Self::Recorder {
        Self::Recorder::new(self)
    }
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
pub struct RecordedTickProposal {
    pub effects: RecordedEffects,
    pub time_passed: Duration,
}

impl<'a> MaybeRecordedProposal for &'a mut RecordedTickProposal {
    type Proposal = TickProposal<'a, RecordedEffects>;

    fn as_proposal(self) -> Self::Proposal {
        Self::Proposal {
            effects: &mut self.effects,
            time_passed: self.time_passed,
        }
    }
}

pub struct TickProposalRecorder<'a, Efs> {
    effects: EffectsRecorder<'a, Efs>,
    time_passed: CloneRecorder<Duration>,
}

impl<'a, Efs> TickProposalRecorder<'a, Efs> {
    pub fn new(proposal: TickProposal<'a, Efs>) -> Self {
        Self {
            effects: EffectsRecorder::new(proposal.effects),
            time_passed: proposal.time_passed.default_recorder(),
        }
    }

    pub fn record<'b>(&'b mut self) -> TickProposal<'b, EffectsRecorder<'a, Efs>> {
        TickProposal {
            effects: self.effects.record(),
            time_passed: self.time_passed.record(),
        }
    }

    pub fn finish_recording(self) -> RecordedTickProposal {
        RecordedTickProposal {
            effects: self.effects.finish_recording(),
            time_passed: self.time_passed.finish_recording(),
        }
    }
}
