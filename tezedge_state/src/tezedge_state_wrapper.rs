use std::fmt::Debug;
use std::time::Instant;

use tla_sm::{Acceptor, GetRequests};

use crate::proposals::MaybeRecordedProposal;
use crate::{PeerAddress, TezedgeConfig, TezedgeRequest, TezedgeState, TezedgeStats};

/// Wrapper around [TezedgeState].
///
/// Wrapper can be used to intercept communication between
/// [tezedge_state::TezedgeState] and [tezedge_state::TezedgeProposer].
#[derive(Debug, Clone)]
pub struct TezedgeStateWrapper {
    state: TezedgeState,
}

impl TezedgeStateWrapper {
    #[inline]
    pub fn newest_time_seen(&self) -> Instant {
        self.state.newest_time_seen()
    }

    #[inline]
    pub fn is_peer_connected(&mut self, peer: &PeerAddress) -> bool {
        self.state.is_peer_connected(peer)
    }

    #[inline]
    pub fn assert_state(&self) {
        self.state.assert_state()
    }

    #[inline]
    pub fn config(&self) -> &TezedgeConfig {
        &self.state.config
    }

    #[inline]
    pub fn stats(&self) -> TezedgeStats {
        self.state.stats()
    }
}

impl<P> Acceptor<P> for TezedgeStateWrapper
where
    P: MaybeRecordedProposal,
    TezedgeState: Acceptor<P::Proposal>,
{
    #[inline]
    fn accept(&mut self, proposal: P) {
        self.state.accept(proposal.as_proposal())
    }
}

impl GetRequests for TezedgeStateWrapper {
    type Request = TezedgeRequest;

    #[inline]
    fn get_requests(&self, buf: &mut Vec<Self::Request>) -> usize {
        self.state.get_requests(buf)
    }
}

impl From<TezedgeState> for TezedgeStateWrapper {
    #[inline]
    fn from(state: TezedgeState) -> Self {
        TezedgeStateWrapper { state }
    }
}
