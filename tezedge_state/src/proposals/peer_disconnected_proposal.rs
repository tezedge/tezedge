// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use tla_sm::{recorders::CloneRecorder, DefaultRecorder, Proposal};

use crate::{EffectsRecorder, PeerAddress, RecordedEffects};

use super::MaybeRecordedProposal;

/// Peer has disconnected.
pub struct PeerDisconnectedProposal<'a, Efs> {
    pub effects: &'a mut Efs,
    pub peer: PeerAddress,
}

impl<'a, Efs> Proposal for PeerDisconnectedProposal<'a, Efs> {}

impl<'a, Efs> DefaultRecorder for PeerDisconnectedProposal<'a, Efs> {
    type Recorder = PeerDisconnectedProposalRecorder<'a, Efs>;

    fn default_recorder(self) -> Self::Recorder {
        Self::Recorder::new(self)
    }
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
pub struct RecordedPeerDisconnectedProposal {
    pub effects: RecordedEffects,
    pub peer: PeerAddress,
}

impl<'a> MaybeRecordedProposal for &'a mut RecordedPeerDisconnectedProposal {
    type Proposal = PeerDisconnectedProposal<'a, RecordedEffects>;

    fn as_proposal(self) -> Self::Proposal {
        Self::Proposal {
            effects: &mut self.effects,
            peer: self.peer,
        }
    }
}

pub struct PeerDisconnectedProposalRecorder<'a, Efs> {
    effects: EffectsRecorder<'a, Efs>,
    peer: CloneRecorder<PeerAddress>,
}

impl<'a, Efs> PeerDisconnectedProposalRecorder<'a, Efs> {
    pub fn new(proposal: PeerDisconnectedProposal<'a, Efs>) -> Self {
        Self {
            effects: EffectsRecorder::new(proposal.effects),
            peer: proposal.peer.default_recorder(),
        }
    }

    pub fn record<'b>(&'b mut self) -> PeerDisconnectedProposal<'b, EffectsRecorder<'a, Efs>> {
        PeerDisconnectedProposal {
            effects: self.effects.record(),
            peer: self.peer.record(),
        }
    }

    pub fn finish_recording(self) -> RecordedPeerDisconnectedProposal {
        RecordedPeerDisconnectedProposal {
            effects: self.effects.finish_recording(),
            peer: self.peer.finish_recording(),
        }
    }
}
