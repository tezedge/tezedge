// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use tezos_messages::p2p::encoding::peer::PeerMessageResponse;
use tla_sm::{recorders::CloneRecorder, DefaultRecorder, Proposal};

use crate::{EffectsRecorder, PeerAddress, RecordedEffects};

use super::MaybeRecordedProposal;

pub struct PeerMessageProposal<'a, Efs> {
    pub effects: &'a mut Efs,
    pub peer: PeerAddress,
    pub message: PeerMessageResponse,
}

impl<'a, Efs> Proposal for PeerMessageProposal<'a, Efs> {}

impl<'a, Efs> DefaultRecorder for PeerMessageProposal<'a, Efs> {
    type Recorder = PeerMessageProposalRecorder<'a, Efs>;

    fn default_recorder(self) -> Self::Recorder {
        Self::Recorder::new(self)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RecordedPeerMessageProposal {
    pub effects: RecordedEffects,
    pub peer: PeerAddress,
    pub message: PeerMessageResponse,
}

impl<'a> MaybeRecordedProposal for &'a mut RecordedPeerMessageProposal {
    type Proposal = PeerMessageProposal<'a, RecordedEffects>;

    fn as_proposal(self) -> Self::Proposal {
        Self::Proposal {
            effects: &mut self.effects,
            peer: self.peer,
            message: self.message.clone(),
        }
    }
}

pub struct PeerMessageProposalRecorder<'a, Efs> {
    effects: EffectsRecorder<'a, Efs>,
    peer: CloneRecorder<PeerAddress>,
    message: CloneRecorder<PeerMessageResponse>,
}

impl<'a, Efs> PeerMessageProposalRecorder<'a, Efs> {
    pub fn new(proposal: PeerMessageProposal<'a, Efs>) -> Self {
        Self {
            effects: EffectsRecorder::new(proposal.effects),
            peer: proposal.peer.default_recorder(),
            message: CloneRecorder::new(proposal.message),
        }
    }

    pub fn record<'b>(&'b mut self) -> PeerMessageProposal<'b, EffectsRecorder<'a, Efs>> {
        PeerMessageProposal {
            effects: self.effects.record(),
            peer: self.peer.record(),
            message: self.message.record(),
        }
    }

    pub fn finish_recording(self) -> RecordedPeerMessageProposal {
        RecordedPeerMessageProposal {
            effects: self.effects.finish_recording(),
            peer: self.peer.finish_recording(),
            message: self.message.finish_recording(),
        }
    }
}
