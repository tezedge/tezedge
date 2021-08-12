use std::time::Instant;
use tla_sm::{recorders::CloneRecorder, DefaultRecorder, Proposal};

use crate::{EffectsRecorder, PeerAddress, RecordedEffects};

use super::MaybeRecordedProposal;

/// Disconnect the peer.
pub struct PeerDisconnectProposal<'a, Efs> {
    pub effects: &'a mut Efs,
    pub at: Instant,
    pub peer: PeerAddress,
}

impl<'a, Efs> Proposal for PeerDisconnectProposal<'a, Efs> {
    fn time(&self) -> Instant {
        self.at
    }
}

impl<'a, Efs> DefaultRecorder for PeerDisconnectProposal<'a, Efs> {
    type Recorder = PeerDisconnectProposalRecorder<'a, Efs>;

    fn default_recorder(self) -> Self::Recorder {
        Self::Recorder::new(self)
    }
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct RecordedPeerDisconnectProposal {
    pub effects: RecordedEffects,
    pub at: Instant,
    pub peer: PeerAddress,
}

impl<'a> MaybeRecordedProposal for &'a mut RecordedPeerDisconnectProposal {
    type Proposal = PeerDisconnectProposal<'a, RecordedEffects>;

    fn as_proposal(self) -> Self::Proposal {
        Self::Proposal {
            effects: &mut self.effects,
            at: self.at,
            peer: self.peer,
        }
    }
}

pub struct PeerDisconnectProposalRecorder<'a, Efs> {
    effects: EffectsRecorder<'a, Efs>,
    at: CloneRecorder<Instant>,
    peer: CloneRecorder<PeerAddress>,
}

impl<'a, Efs> PeerDisconnectProposalRecorder<'a, Efs> {
    pub fn new(proposal: PeerDisconnectProposal<'a, Efs>) -> Self {
        Self {
            effects: EffectsRecorder::new(proposal.effects),
            at: proposal.at.default_recorder(),
            peer: proposal.peer.default_recorder(),
        }
    }

    pub fn record<'b>(&'b mut self) -> PeerDisconnectProposal<'b, EffectsRecorder<'a, Efs>> {
        PeerDisconnectProposal {
            effects: self.effects.record(),
            at: self.at.record(),
            peer: self.peer.record(),
        }
    }

    pub fn finish_recording(self) -> RecordedPeerDisconnectProposal {
        RecordedPeerDisconnectProposal {
            effects: self.effects.finish_recording(),
            at: self.at.finish_recording(),
            peer: self.peer.finish_recording(),
        }
    }
}
