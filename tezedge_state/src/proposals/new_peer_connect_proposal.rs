use std::time::Instant;
use tla_sm::{recorders::CloneRecorder, DefaultRecorder, Proposal};

use crate::{EffectsRecorder, PeerAddress, RecordedEffects};

use super::MaybeRecordedProposal;

pub struct NewPeerConnectProposal<'a, Efs> {
    pub effects: &'a mut Efs,
    pub at: Instant,
    pub peer: PeerAddress,
}

impl<'a, Efs> Proposal for NewPeerConnectProposal<'a, Efs> {
    fn time(&self) -> Instant {
        self.at
    }
}

impl<'a, Efs> DefaultRecorder for NewPeerConnectProposal<'a, Efs> {
    type Recorder = NewPeerConnectProposalRecorder<'a, Efs>;

    fn default_recorder(self) -> Self::Recorder {
        Self::Recorder::new(self)
    }
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct RecordedNewPeerConnectProposal {
    pub effects: RecordedEffects,
    pub at: Instant,
    pub peer: PeerAddress,
}

impl<'a> MaybeRecordedProposal for &'a mut RecordedNewPeerConnectProposal {
    type Proposal = NewPeerConnectProposal<'a, RecordedEffects>;

    fn as_proposal(self) -> Self::Proposal {
        Self::Proposal {
            effects: &mut self.effects,
            at: self.at,
            peer: self.peer,
        }
    }
}

pub struct NewPeerConnectProposalRecorder<'a, Efs> {
    effects: EffectsRecorder<'a, Efs>,
    at: CloneRecorder<Instant>,
    peer: CloneRecorder<PeerAddress>,
}

impl<'a, Efs> NewPeerConnectProposalRecorder<'a, Efs> {
    pub fn new(proposal: NewPeerConnectProposal<'a, Efs>) -> Self {
        Self {
            effects: EffectsRecorder::new(proposal.effects),
            at: proposal.at.default_recorder(),
            peer: proposal.peer.default_recorder(),
        }
    }

    pub fn record<'b>(&'b mut self) -> NewPeerConnectProposal<'b, EffectsRecorder<'a, Efs>> {
        NewPeerConnectProposal {
            effects: self.effects.record(),
            at: self.at.record(),
            peer: self.peer.record(),
        }
    }

    pub fn finish_recording(self) -> RecordedNewPeerConnectProposal {
        RecordedNewPeerConnectProposal {
            effects: self.effects.finish_recording(),
            at: self.at.finish_recording(),
            peer: self.peer.finish_recording(),
        }
    }
}
