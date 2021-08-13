use std::fmt::{self, Debug};
use std::time::Duration;
use tezos_messages::p2p::encoding::peer::PeerMessage;
use tla_sm::recorders::CloneRecorder;
use tla_sm::{DefaultRecorder, Proposal};

use crate::{EffectsRecorder, PeerAddress, RecordedEffects};

use super::MaybeRecordedProposal;

pub struct SendPeerMessageProposal<'a, Efs> {
    pub effects: &'a mut Efs,
    pub time_passed: Duration,
    pub peer: PeerAddress,
    pub message: PeerMessage,
}

impl<'a, Efs> Debug for SendPeerMessageProposal<'a, Efs> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PeerSendMessageProposal")
            .field("time_passed", &self.time_passed)
            .field("peer", &self.peer)
            .finish()
    }
}

impl<'a, Efs> Proposal for SendPeerMessageProposal<'a, Efs> {
    fn time_passed(&self) -> Duration {
        self.time_passed
    }

    fn nullify_time_passed(&mut self) {
        self.time_passed = Duration::new(0, 0);
    }
}

impl<'a, Efs> DefaultRecorder for SendPeerMessageProposal<'a, Efs> {
    type Recorder = SendPeerMessageProposalRecorder<'a, Efs>;

    fn default_recorder(self) -> Self::Recorder {
        Self::Recorder::new(self)
    }
}

#[derive(Debug, Clone)]
pub struct RecordedSendPeerMessageProposal {
    pub effects: RecordedEffects,
    pub time_passed: Duration,
    pub peer: PeerAddress,
    pub message: PeerMessage,
}

impl<'a> MaybeRecordedProposal for &'a mut RecordedSendPeerMessageProposal {
    type Proposal = SendPeerMessageProposal<'a, RecordedEffects>;

    fn as_proposal(self) -> Self::Proposal {
        Self::Proposal {
            effects: &mut self.effects,
            time_passed: self.time_passed,
            peer: self.peer,
            message: self.message.clone(),
        }
    }
}

pub struct SendPeerMessageProposalRecorder<'a, Efs> {
    effects: EffectsRecorder<'a, Efs>,
    time_passed: CloneRecorder<Duration>,
    peer: CloneRecorder<PeerAddress>,
    message: CloneRecorder<PeerMessage>,
}

impl<'a, Efs> SendPeerMessageProposalRecorder<'a, Efs> {
    pub fn new(proposal: SendPeerMessageProposal<'a, Efs>) -> Self {
        Self {
            effects: EffectsRecorder::new(proposal.effects),
            time_passed: proposal.time_passed.default_recorder(),
            peer: proposal.peer.default_recorder(),
            message: CloneRecorder::new(proposal.message),
        }
    }

    pub fn record<'b>(&'b mut self) -> SendPeerMessageProposal<'b, EffectsRecorder<'a, Efs>> {
        SendPeerMessageProposal {
            effects: self.effects.record(),
            time_passed: self.time_passed.record(),
            peer: self.peer.record(),
            message: self.message.record(),
        }
    }

    pub fn finish_recording(self) -> RecordedSendPeerMessageProposal {
        RecordedSendPeerMessageProposal {
            effects: self.effects.finish_recording(),
            time_passed: self.time_passed.finish_recording(),
            peer: self.peer.finish_recording(),
            message: self.message.finish_recording(),
        }
    }
}
