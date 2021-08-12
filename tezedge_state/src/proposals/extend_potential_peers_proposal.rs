use std::{net::SocketAddr, time::Instant};
use tla_sm::{
    recorders::{CloneRecorder, IteratorRecorder, RecordedIterator},
    DefaultRecorder, Proposal,
};

use crate::{EffectsRecorder, RecordedEffects};

use super::MaybeRecordedProposal;

pub struct ExtendPotentialPeersProposal<'a, Efs, P> {
    pub effects: &'a mut Efs,
    pub at: Instant,
    pub peers: P,
}

impl<'a, Efs, P> Proposal for ExtendPotentialPeersProposal<'a, Efs, P> {
    fn time(&self) -> Instant {
        self.at
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

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct RecordedExtendPotentialPeersProposal {
    pub effects: RecordedEffects,
    pub at: Instant,
    pub peers: Vec<SocketAddr>,
}

impl<'a> MaybeRecordedProposal for &'a mut RecordedExtendPotentialPeersProposal {
    type Proposal = ExtendPotentialPeersProposal<'a, RecordedEffects, RecordedIterator<SocketAddr>>;

    fn as_proposal(self) -> Self::Proposal {
        Self::Proposal {
            effects: &mut self.effects,
            at: self.at,
            peers: self.peers.clone(),
        }
    }
}

pub struct ExtendPotentialPeersProposalRecorder<'a, Efs, I> {
    effects: EffectsRecorder<'a, Efs>,
    at: CloneRecorder<Instant>,
    peers: IteratorRecorder<I, SocketAddr>,
}

impl<'a, Efs, I> ExtendPotentialPeersProposalRecorder<'a, Efs, I> {
    pub fn new<P>(proposal: ExtendPotentialPeersProposal<'a, Efs, P>) -> Self
    where
        P: IntoIterator<Item = SocketAddr, IntoIter = I>,
    {
        Self {
            effects: EffectsRecorder::new(proposal.effects),
            at: proposal.at.default_recorder(),
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
            at: self.at.record(),
            peers: self.peers.record(),
        }
    }

    pub fn finish_recording(self) -> RecordedExtendPotentialPeersProposal {
        RecordedExtendPotentialPeersProposal {
            effects: self.effects.finish_recording(),
            at: self.at.finish_recording(),
            peers: self.peers.finish_recording(),
        }
    }
}
