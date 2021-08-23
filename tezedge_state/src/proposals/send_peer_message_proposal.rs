// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::fmt::{self, Debug};
use tezos_messages::p2p::encoding::peer::PeerMessage;
use tla_sm::recorders::CloneRecorder;
use tla_sm::{DefaultRecorder, Proposal};

use crate::{EffectsRecorder, PeerAddress, RecordedEffects};

use super::MaybeRecordedProposal;

pub struct SendPeerMessageProposal<'a, Efs> {
    pub effects: &'a mut Efs,
    pub peer: PeerAddress,
    pub message: PeerMessage,
}

impl<'a, Efs> Debug for SendPeerMessageProposal<'a, Efs> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PeerSendMessageProposal")
            .field("peer", &self.peer)
            .finish()
    }
}

impl<'a, Efs> Proposal for SendPeerMessageProposal<'a, Efs> {}

impl<'a, Efs> DefaultRecorder for SendPeerMessageProposal<'a, Efs> {
    type Recorder = SendPeerMessageProposalRecorder<'a, Efs>;

    fn default_recorder(self) -> Self::Recorder {
        Self::Recorder::new(self)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RecordedSendPeerMessageProposal {
    pub effects: RecordedEffects,
    pub peer: PeerAddress,
    pub message: PeerMessage,
}

impl<'a> MaybeRecordedProposal for &'a mut RecordedSendPeerMessageProposal {
    type Proposal = SendPeerMessageProposal<'a, RecordedEffects>;

    fn as_proposal(self) -> Self::Proposal {
        Self::Proposal {
            effects: &mut self.effects,
            peer: self.peer,
            message: self.message.clone(),
        }
    }
}

pub struct SendPeerMessageProposalRecorder<'a, Efs> {
    effects: EffectsRecorder<'a, Efs>,
    peer: CloneRecorder<PeerAddress>,
    message: CloneRecorder<PeerMessage>,
}

impl<'a, Efs> SendPeerMessageProposalRecorder<'a, Efs> {
    pub fn new(proposal: SendPeerMessageProposal<'a, Efs>) -> Self {
        Self {
            effects: EffectsRecorder::new(proposal.effects),
            peer: proposal.peer.default_recorder(),
            message: CloneRecorder::new(proposal.message),
        }
    }

    pub fn record<'b>(&'b mut self) -> SendPeerMessageProposal<'b, EffectsRecorder<'a, Efs>> {
        SendPeerMessageProposal {
            effects: self.effects.record(),
            peer: self.peer.record(),
            message: self.message.record(),
        }
    }

    pub fn finish_recording(self) -> RecordedSendPeerMessageProposal {
        RecordedSendPeerMessageProposal {
            effects: self.effects.finish_recording(),
            peer: self.peer.finish_recording(),
            message: self.message.finish_recording(),
        }
    }
}
