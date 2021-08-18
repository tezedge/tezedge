// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::time::Duration;
use tla_sm::{recorders::CloneRecorder, DefaultRecorder, Proposal};

use crate::{EffectsRecorder, RecordedEffects};

use super::MaybeRecordedProposal;

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
pub enum PendingRequestMsg {
    StartListeningForNewPeersError {
        #[serde(with = "tla_sm::serde_io_error_kind")]
        error: std::io::ErrorKind,
    },
    StartListeningForNewPeersSuccess,

    StopListeningForNewPeersSuccess,

    SendPeerAckPending,
    SendPeerAckSuccess,

    ConnectPeerPending,
    ConnectPeerSuccess,
    ConnectPeerError,

    DisconnectPeerPending,
    DisconnectPeerSuccess,

    BlacklistPeerPending,
    BlacklistPeerSuccess,

    PeerMessageReceivedNotified,

    /// Handshake which was successful was notified.
    HandshakeSuccessfulNotified,
}

impl DefaultRecorder for PendingRequestMsg {
    type Recorder = CloneRecorder<PendingRequestMsg>;

    fn default_recorder(self) -> Self::Recorder {
        Self::Recorder::new(self)
    }
}

pub struct PendingRequestProposal<'a, Efs> {
    pub effects: &'a mut Efs,
    pub time_passed: Duration,
    pub req_id: usize,
    pub message: PendingRequestMsg,
}

impl<'a, Efs> Proposal for PendingRequestProposal<'a, Efs> {
    fn time_passed(&self) -> Duration {
        self.time_passed
    }

    fn nullify_time_passed(&mut self) {
        self.time_passed = Duration::new(0, 0);
    }
}

impl<'a, Efs> DefaultRecorder for PendingRequestProposal<'a, Efs> {
    type Recorder = PendingRequestProposalRecorder<'a, Efs>;

    fn default_recorder(self) -> Self::Recorder {
        Self::Recorder::new(self)
    }
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
pub struct RecordedPendingRequestProposal {
    pub effects: RecordedEffects,
    pub time_passed: Duration,
    pub req_id: usize,
    pub message: PendingRequestMsg,
}

impl<'a> MaybeRecordedProposal for &'a mut RecordedPendingRequestProposal {
    type Proposal = PendingRequestProposal<'a, RecordedEffects>;

    fn as_proposal(self) -> Self::Proposal {
        Self::Proposal {
            effects: &mut self.effects,
            time_passed: self.time_passed,
            req_id: self.req_id,
            message: self.message.clone(),
        }
    }
}

pub struct PendingRequestProposalRecorder<'a, Efs> {
    effects: EffectsRecorder<'a, Efs>,
    time_passed: CloneRecorder<Duration>,
    req_id: CloneRecorder<usize>,
    message: CloneRecorder<PendingRequestMsg>,
}

impl<'a, Efs> PendingRequestProposalRecorder<'a, Efs> {
    pub fn new(proposal: PendingRequestProposal<'a, Efs>) -> Self {
        Self {
            effects: EffectsRecorder::new(proposal.effects),
            time_passed: proposal.time_passed.default_recorder(),
            req_id: proposal.req_id.default_recorder(),
            message: proposal.message.default_recorder(),
        }
    }

    pub fn record<'b>(&'b mut self) -> PendingRequestProposal<'b, EffectsRecorder<'a, Efs>> {
        PendingRequestProposal {
            effects: self.effects.record(),
            time_passed: self.time_passed.record(),
            req_id: self.req_id.record(),
            message: self.message.record(),
        }
    }

    pub fn finish_recording(self) -> RecordedPendingRequestProposal {
        RecordedPendingRequestProposal {
            effects: self.effects.finish_recording(),
            time_passed: self.time_passed.finish_recording(),
            req_id: self.req_id.finish_recording(),
            message: self.message.finish_recording(),
        }
    }
}
