// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::time::Duration;
use tla_sm::{
    recorders::{CloneRecorder, IteratorRecorder, RecordedIterator},
    DefaultRecorder, Proposal,
};

use crate::{EffectsRecorder, RecordedEffects};

use super::MaybeRecordedProposal;

pub struct ExtendPotentialPeersProposal<'a, Efs, P> {
    pub effects: &'a mut Efs,
    pub time_passed: Duration,
    pub peers: P,
}

impl<'a, Efs, P> Proposal for ExtendPotentialPeersProposal<'a, Efs, P> {
    fn time_passed(&self) -> Duration {
        self.time_passed
    }

    fn nullify_time_passed(&mut self) {
        self.time_passed = Duration::new(0, 0);
    }
}

impl<'a, Efs, P> DefaultRecorder for ExtendPotentialPeersProposal<'a, Efs, P>
where
    P: 'a + IntoIterator<Item = SocketAddr>,
{
    type Recorder = ExtendPotentialPeersProposalRecorder<'a, Efs, P::IntoIter>;

    fn default_recorder(self) -> Self::Recorder {
        Self::Recorder::new(self)
    }
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
pub struct RecordedExtendPotentialPeersProposal {
    pub effects: RecordedEffects,
    pub time_passed: Duration,
    pub peers: Vec<SocketAddr>,
}

impl<'a> MaybeRecordedProposal for &'a mut RecordedExtendPotentialPeersProposal {
    type Proposal = ExtendPotentialPeersProposal<'a, RecordedEffects, RecordedIterator<SocketAddr>>;

    fn as_proposal(self) -> Self::Proposal {
        Self::Proposal {
            effects: &mut self.effects,
            time_passed: self.time_passed,
            peers: self.peers.clone(),
        }
    }
}

pub struct ExtendPotentialPeersProposalRecorder<'a, Efs, I> {
    effects: EffectsRecorder<'a, Efs>,
    time_passed: CloneRecorder<Duration>,
    peers: IteratorRecorder<I, SocketAddr>,
}

impl<'a, Efs, I> ExtendPotentialPeersProposalRecorder<'a, Efs, I> {
    pub fn new<P>(proposal: ExtendPotentialPeersProposal<'a, Efs, P>) -> Self
    where
        P: IntoIterator<Item = SocketAddr, IntoIter = I>,
    {
        Self {
            effects: EffectsRecorder::new(proposal.effects),
            time_passed: proposal.time_passed.default_recorder(),
            peers: IteratorRecorder::new(proposal.peers.into_iter()),
        }
    }

    pub fn record<'b>(
        &'b mut self,
    ) -> ExtendPotentialPeersProposal<
        'b,
        EffectsRecorder<'a, Efs>,
        &'b mut IteratorRecorder<I, SocketAddr>,
    > {
        ExtendPotentialPeersProposal {
            effects: self.effects.record(),
            time_passed: self.time_passed.record(),
            peers: self.peers.record(),
        }
    }

    pub fn finish_recording(self) -> RecordedExtendPotentialPeersProposal {
        RecordedExtendPotentialPeersProposal {
            effects: self.effects.finish_recording(),
            time_passed: self.time_passed.finish_recording(),
            peers: self.peers.finish_recording(),
        }
    }
}
