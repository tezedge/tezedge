use std::time::Instant;
use tla_sm::{recorders::CloneRecorder, DefaultRecorder, Proposal};

use crate::{EffectsRecorder, PeerAddress, RecordedEffects};

use super::MaybeRecordedProposal;

pub struct PeerBlacklistProposal<'a, Efs> {
    pub effects: &'a mut Efs,
    pub at: Instant,
    pub peer: PeerAddress,
}

impl<'a, Efs> Proposal for PeerBlacklistProposal<'a, Efs> {
    fn time(&self) -> Instant {
        self.at
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
    pub at: Instant,
    pub peer: PeerAddress,
}

impl<'a> MaybeRecordedProposal for &'a mut RecordedPeerBlacklistProposal {
    type Proposal = PeerBlacklistProposal<'a, RecordedEffects>;

    fn as_proposal(self) -> Self::Proposal {
        Self::Proposal {
            effects: &mut self.effects,
            at: self.at,
            peer: self.peer,
        }
    }
}

pub struct PeerBlacklistProposalRecorder<'a, Efs> {
    effects: EffectsRecorder<'a, Efs>,
    at: CloneRecorder<Instant>,
    peer: CloneRecorder<PeerAddress>,
}

impl<'a, Efs> PeerBlacklistProposalRecorder<'a, Efs> {
    pub fn new(proposal: PeerBlacklistProposal<'a, Efs>) -> Self {
        Self {
            effects: EffectsRecorder::new(proposal.effects),
            at: proposal.at.default_recorder(),
            peer: proposal.peer.default_recorder(),
        }
    }

    pub fn record<'b>(&'b mut self) -> PeerBlacklistProposal<'b, EffectsRecorder<'a, Efs>> {
        PeerBlacklistProposal {
            effects: self.effects.record(),
            at: self.at.record(),
            peer: self.peer.record(),
        }
    }

    pub fn finish_recording(self) -> RecordedPeerBlacklistProposal {
        RecordedPeerBlacklistProposal {
            effects: self.effects.finish_recording(),
            at: self.at.finish_recording(),
            peer: self.peer.finish_recording(),
        }
    }
}
