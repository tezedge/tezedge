use std::time::Duration;
use tla_sm::{recorders::CloneRecorder, DefaultRecorder, Proposal};

use crate::{EffectsRecorder, PeerAddress, RecordedEffects};

use super::MaybeRecordedProposal;

pub struct PeerBlacklistProposal<'a, Efs> {
    pub effects: &'a mut Efs,
    pub time_passed: Duration,
    pub peer: PeerAddress,
}

impl<'a, Efs> Proposal for PeerBlacklistProposal<'a, Efs> {
    fn time_passed(&self) -> Duration {
        self.time_passed
    }

    fn nullify_time_passed(&mut self) {
        self.time_passed = Duration::new(0, 0);
    }
}

impl<'a, Efs> DefaultRecorder for PeerBlacklistProposal<'a, Efs> {
    type Recorder = PeerBlacklistProposalRecorder<'a, Efs>;

    fn default_recorder(self) -> Self::Recorder {
        Self::Recorder::new(self)
    }
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct RecordedPeerBlacklistProposal {
    pub effects: RecordedEffects,
    pub time_passed: Duration,
    pub peer: PeerAddress,
}

impl<'a> MaybeRecordedProposal for &'a mut RecordedPeerBlacklistProposal {
    type Proposal = PeerBlacklistProposal<'a, RecordedEffects>;

    fn as_proposal(self) -> Self::Proposal {
        Self::Proposal {
            effects: &mut self.effects,
            time_passed: self.time_passed,
            peer: self.peer,
        }
    }
}

pub struct PeerBlacklistProposalRecorder<'a, Efs> {
    effects: EffectsRecorder<'a, Efs>,
    time_passed: CloneRecorder<Duration>,
    peer: CloneRecorder<PeerAddress>,
}

impl<'a, Efs> PeerBlacklistProposalRecorder<'a, Efs> {
    pub fn new(proposal: PeerBlacklistProposal<'a, Efs>) -> Self {
        Self {
            effects: EffectsRecorder::new(proposal.effects),
            time_passed: proposal.time_passed.default_recorder(),
            peer: proposal.peer.default_recorder(),
        }
    }

    pub fn record<'b>(&'b mut self) -> PeerBlacklistProposal<'b, EffectsRecorder<'a, Efs>> {
        PeerBlacklistProposal {
            effects: self.effects.record(),
            time_passed: self.time_passed.record(),
            peer: self.peer.record(),
        }
    }

    pub fn finish_recording(self) -> RecordedPeerBlacklistProposal {
        RecordedPeerBlacklistProposal {
            effects: self.effects.finish_recording(),
            time_passed: self.time_passed.finish_recording(),
            peer: self.peer.finish_recording(),
        }
    }
}
