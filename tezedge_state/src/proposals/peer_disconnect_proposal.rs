// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use tla_sm::{recorders::CloneRecorder, DefaultRecorder, Proposal};

use crate::{EffectsRecorder, PeerAddress, RecordedEffects};

use super::MaybeRecordedProposal;

/// Disconnect the peer.
pub struct PeerDisconnectProposal<'a, Efs> {
    pub effects: &'a mut Efs,
    pub peer: PeerAddress,
}

impl<'a, Efs> Proposal for PeerDisconnectProposal<'a, Efs> {}

impl<'a, Efs> DefaultRecorder for PeerDisconnectProposal<'a, Efs> {
    type Recorder = PeerDisconnectProposalRecorder<'a, Efs>;

    fn default_recorder(self) -> Self::Recorder {
        Self::Recorder::new(self)
    }
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
pub struct RecordedPeerDisconnectProposal {
    pub effects: RecordedEffects,
    pub peer: PeerAddress,
}

impl<'a> MaybeRecordedProposal for &'a mut RecordedPeerDisconnectProposal {
    type Proposal = PeerDisconnectProposal<'a, RecordedEffects>;

    fn as_proposal(self) -> Self::Proposal {
        Self::Proposal {
            effects: &mut self.effects,
            peer: self.peer,
        }
    }
}

pub struct PeerDisconnectProposalRecorder<'a, Efs> {
    effects: EffectsRecorder<'a, Efs>,
    peer: CloneRecorder<PeerAddress>,
}

impl<'a, Efs> PeerDisconnectProposalRecorder<'a, Efs> {
    pub fn new(proposal: PeerDisconnectProposal<'a, Efs>) -> Self {
        Self {
            effects: EffectsRecorder::new(proposal.effects),
            peer: proposal.peer.default_recorder(),
        }
    }

    pub fn record<'b>(&'b mut self) -> PeerDisconnectProposal<'b, EffectsRecorder<'a, Efs>> {
        PeerDisconnectProposal {
            effects: self.effects.record(),
            peer: self.peer.record(),
        }
    }

    pub fn finish_recording(self) -> RecordedPeerDisconnectProposal {
        RecordedPeerDisconnectProposal {
            effects: self.effects.finish_recording(),
            peer: self.peer.finish_recording(),
        }
    }
}
