use std::fmt::{self, Debug};
use std::time::Instant;
use tla_sm::recorders::{CloneRecorder, RecordedStream, StreamRecorder};
use tla_sm::{DefaultRecorder, Proposal};

use crate::{EffectsRecorder, PeerAddress, RecordedEffects};

use super::{MaybeRecordedProposal, PeerWritableProposal};

pub struct PeerReadableProposal<'a, Efs, S> {
    pub effects: &'a mut Efs,
    pub at: Instant,
    pub peer: PeerAddress,
    pub stream: &'a mut S,
}

impl<'a, Efs, S> Debug for PeerReadableProposal<'a, Efs, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PeerReadableProposal")
            .field("at", &self.at)
            .field("peer", &self.peer)
            .finish()
    }
}

impl<'a, Efs, S> Proposal for PeerReadableProposal<'a, Efs, S> {
    fn time(&self) -> Instant {
        self.at
    }
}

impl<'a, Efs, S> From<PeerWritableProposal<'a, Efs, S>> for PeerReadableProposal<'a, Efs, S> {
    fn from(proposal: PeerWritableProposal<'a, Efs, S>) -> Self {
        Self {
            effects: proposal.effects,
            at: proposal.at,
            peer: proposal.peer,
            stream: proposal.stream,
        }
    }
}

impl<'a, Efs, S> DefaultRecorder for PeerReadableProposal<'a, Efs, S> {
    type Recorder = PeerReadableProposalRecorder<'a, Efs, S>;

    fn default_recorder(self) -> Self::Recorder {
        Self::Recorder::new(self)
    }
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct RecordedPeerReadableProposal {
    pub effects: RecordedEffects,
    pub at: Instant,
    pub peer: PeerAddress,
    pub stream: RecordedStream,
}

impl<'a> MaybeRecordedProposal for &'a mut RecordedPeerReadableProposal {
    type Proposal = PeerReadableProposal<'a, RecordedEffects, RecordedStream>;

    fn as_proposal(self) -> Self::Proposal {
        Self::Proposal {
            effects: &mut self.effects,
            at: self.at,
            peer: self.peer,
            stream: &mut self.stream,
        }
    }
}

pub struct PeerReadableProposalRecorder<'a, Efs, S> {
    effects: EffectsRecorder<'a, Efs>,
    at: CloneRecorder<Instant>,
    peer: CloneRecorder<PeerAddress>,
    stream: StreamRecorder<&'a mut S>,
}

impl<'a, Efs, S> PeerReadableProposalRecorder<'a, Efs, S> {
    pub fn new(proposal: PeerReadableProposal<'a, Efs, S>) -> Self {
        Self {
            effects: EffectsRecorder::new(proposal.effects),
            at: proposal.at.default_recorder(),
            peer: proposal.peer.default_recorder(),
            stream: StreamRecorder::new(proposal.stream),
        }
    }

    pub fn record<'b>(
        &'b mut self,
    ) -> PeerReadableProposal<'b, EffectsRecorder<'a, Efs>, StreamRecorder<&'a mut S>> {
        PeerReadableProposal {
            effects: self.effects.record(),
            at: self.at.record(),
            peer: self.peer.record(),
            stream: self.stream.record(),
        }
    }

    pub fn finish_recording(self) -> RecordedPeerReadableProposal {
        RecordedPeerReadableProposal {
            effects: self.effects.finish_recording(),
            at: self.at.finish_recording(),
            peer: self.peer.finish_recording(),
            stream: self.stream.finish_recording(),
        }
    }
}
