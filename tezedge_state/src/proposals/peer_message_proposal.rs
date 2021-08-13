use serde::{Deserialize, Serialize};
use std::time::Duration;

use tezos_messages::p2p::encoding::peer::PeerMessageResponse;
use tla_sm::{recorders::CloneRecorder, DefaultRecorder, Proposal};

use crate::{EffectsRecorder, PeerAddress, RecordedEffects};

use super::MaybeRecordedProposal;

pub struct PeerMessageProposal<'a, Efs> {
    pub effects: &'a mut Efs,
    pub time_passed: Duration,
    pub peer: PeerAddress,
    pub message: PeerMessageResponse,
}

impl<'a, Efs> Proposal for PeerMessageProposal<'a, Efs> {
    fn time_passed(&self) -> Duration {
        self.time_passed
    }

    fn nullify_time_passed(&mut self) {
        self.time_passed = Duration::new(0, 0);
    }
}

impl<'a, Efs> DefaultRecorder for PeerMessageProposal<'a, Efs> {
    type Recorder = PeerMessageProposalRecorder<'a, Efs>;

    fn default_recorder(self) -> Self::Recorder {
        Self::Recorder::new(self)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RecordedPeerMessageProposal {
    pub effects: RecordedEffects,
    pub time_passed: Duration,
    pub peer: PeerAddress,
    pub message: PeerMessageResponse,
}

impl<'a> MaybeRecordedProposal for &'a mut RecordedPeerMessageProposal {
    type Proposal = PeerMessageProposal<'a, RecordedEffects>;

    fn as_proposal(self) -> Self::Proposal {
        Self::Proposal {
            effects: &mut self.effects,
            time_passed: self.time_passed,
            peer: self.peer,
            message: self.message.clone(),
        }
    }
}

pub struct PeerMessageProposalRecorder<'a, Efs> {
    effects: EffectsRecorder<'a, Efs>,
    time_passed: CloneRecorder<Duration>,
    peer: CloneRecorder<PeerAddress>,
    message: CloneRecorder<PeerMessageResponse>,
}

impl<'a, Efs> PeerMessageProposalRecorder<'a, Efs> {
    pub fn new(proposal: PeerMessageProposal<'a, Efs>) -> Self {
        Self {
            effects: EffectsRecorder::new(proposal.effects),
            time_passed: proposal.time_passed.default_recorder(),
            peer: proposal.peer.default_recorder(),
            message: CloneRecorder::new(proposal.message),
        }
    }

    pub fn record<'b>(&'b mut self) -> PeerMessageProposal<'b, EffectsRecorder<'a, Efs>> {
        PeerMessageProposal {
            effects: self.effects.record(),
            time_passed: self.time_passed.record(),
            peer: self.peer.record(),
            message: self.message.record(),
        }
    }

    pub fn finish_recording(self) -> RecordedPeerMessageProposal {
        RecordedPeerMessageProposal {
            effects: self.effects.finish_recording(),
            time_passed: self.time_passed.finish_recording(),
            peer: self.peer.finish_recording(),
            message: self.message.finish_recording(),
        }
    }
}
