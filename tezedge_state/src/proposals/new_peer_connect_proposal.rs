// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use tla_sm::{recorders::CloneRecorder, DefaultRecorder, Proposal};

use crate::{EffectsRecorder, PeerAddress, RecordedEffects};

use super::MaybeRecordedProposal;

pub struct NewPeerConnectProposal<'a, Efs> {
    pub effects: &'a mut Efs,
    pub peer: PeerAddress,
}

impl<'a, Efs> Proposal for NewPeerConnectProposal<'a, Efs> {}

impl<'a, Efs> DefaultRecorder for NewPeerConnectProposal<'a, Efs> {
    type Recorder = NewPeerConnectProposalRecorder<'a, Efs>;

    fn default_recorder(self) -> Self::Recorder {
        Self::Recorder::new(self)
    }
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
pub struct RecordedNewPeerConnectProposal {
    pub effects: RecordedEffects,
    pub peer: PeerAddress,
}

impl<'a> MaybeRecordedProposal for &'a mut RecordedNewPeerConnectProposal {
    type Proposal = NewPeerConnectProposal<'a, RecordedEffects>;

    fn as_proposal(self) -> Self::Proposal {
        Self::Proposal {
            effects: &mut self.effects,
            peer: self.peer,
        }
    }
}

pub struct NewPeerConnectProposalRecorder<'a, Efs> {
    effects: EffectsRecorder<'a, Efs>,
    peer: CloneRecorder<PeerAddress>,
}

impl<'a, Efs> NewPeerConnectProposalRecorder<'a, Efs> {
    pub fn new(proposal: NewPeerConnectProposal<'a, Efs>) -> Self {
        Self {
            effects: EffectsRecorder::new(proposal.effects),
            peer: proposal.peer.default_recorder(),
        }
    }

    pub fn record<'b>(&'b mut self) -> NewPeerConnectProposal<'b, EffectsRecorder<'a, Efs>> {
        NewPeerConnectProposal {
            effects: self.effects.record(),
            peer: self.peer.record(),
        }
    }

    pub fn finish_recording(self) -> RecordedNewPeerConnectProposal {
        RecordedNewPeerConnectProposal {
            effects: self.effects.finish_recording(),
            peer: self.peer.finish_recording(),
        }
    }
}
