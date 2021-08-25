// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::fmt::{self, Debug};
use tla_sm::recorders::{CloneRecorder, RecordedStream, StreamRecorder};
use tla_sm::{DefaultRecorder, Proposal};

use crate::{EffectsRecorder, PeerAddress, RecordedEffects};

use super::{MaybeRecordedProposal, PeerReadableProposal};

pub struct PeerWritableProposal<'a, Efs, S> {
    pub effects: &'a mut Efs,
    pub peer: PeerAddress,
    pub stream: &'a mut S,
}

impl<'a, Efs, S> Debug for PeerWritableProposal<'a, Efs, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PeerWritableProposal")
            .field("peer", &self.peer)
            .finish()
    }
}

impl<'a, Efs, S> Proposal for PeerWritableProposal<'a, Efs, S> {}

impl<'a, Efs, S> From<PeerReadableProposal<'a, Efs, S>> for PeerWritableProposal<'a, Efs, S> {
    fn from(proposal: PeerReadableProposal<'a, Efs, S>) -> Self {
        Self {
            effects: proposal.effects,
            peer: proposal.peer,
            stream: proposal.stream,
        }
    }
}

impl<'a, Efs, S> DefaultRecorder for PeerWritableProposal<'a, Efs, S> {
    type Recorder = PeerWritableProposalRecorder<'a, Efs, S>;

    fn default_recorder(self) -> Self::Recorder {
        Self::Recorder::new(self)
    }
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
pub struct RecordedPeerWritableProposal {
    pub effects: RecordedEffects,
    pub peer: PeerAddress,
    pub stream: RecordedStream,
}

impl<'a> MaybeRecordedProposal for &'a mut RecordedPeerWritableProposal {
    type Proposal = PeerWritableProposal<'a, RecordedEffects, RecordedStream>;

    fn as_proposal(self) -> Self::Proposal {
        Self::Proposal {
            effects: &mut self.effects,
            peer: self.peer,
            stream: &mut self.stream,
        }
    }
}

pub struct PeerWritableProposalRecorder<'a, Efs, S> {
    effects: EffectsRecorder<'a, Efs>,
    peer: CloneRecorder<PeerAddress>,
    stream: StreamRecorder<&'a mut S>,
}

impl<'a, Efs, S> PeerWritableProposalRecorder<'a, Efs, S> {
    pub fn new(proposal: PeerWritableProposal<'a, Efs, S>) -> Self {
        Self {
            effects: EffectsRecorder::new(proposal.effects),
            peer: proposal.peer.default_recorder(),
            stream: StreamRecorder::new(proposal.stream),
        }
    }

    pub fn record<'b>(
        &'b mut self,
    ) -> PeerWritableProposal<'b, EffectsRecorder<'a, Efs>, StreamRecorder<&'a mut S>> {
        PeerWritableProposal {
            effects: self.effects.record(),
            peer: self.peer.record(),
            stream: self.stream.record(),
        }
    }

    pub fn finish_recording(self) -> RecordedPeerWritableProposal {
        RecordedPeerWritableProposal {
            effects: self.effects.finish_recording(),
            peer: self.peer.finish_recording(),
            stream: self.stream.finish_recording(),
        }
    }
}
