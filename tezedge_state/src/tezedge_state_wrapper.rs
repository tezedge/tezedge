use std::fmt::Debug;
use std::time::Instant;

use tla_sm::GetRequests;
use tla_sm::{Acceptor, Proposal};

use rand::rngs::ThreadRng;

use crate::{PeerAddress, TezedgeConfig, TezedgeRequest, TezedgeState, TezedgeStats};

/// Wrapper around [TezedgeState].
///
/// Wrapper can be used to intercept communication between
/// [tezedge_state::TezedgeState] and [tezedge_state::TezedgeProposer].
#[derive(Debug, Clone)]
pub struct TezedgeStateWrapper(TezedgeState);

impl TezedgeStateWrapper {
    #[inline]
    pub fn newest_time_seen(&self) -> Instant {
        self.0.newest_time_seen()
    }

    #[inline]
    pub fn is_peer_connected(&mut self, peer: &PeerAddress) -> bool {
        self.0.is_peer_connected(peer)
    }

    #[inline]
    pub fn assert_state(&self) {
        self.0.assert_state()
    }

    #[inline]
    pub fn config(&self) -> &TezedgeConfig {
        &self.0.config
    }

    #[inline]
    pub fn stats(&self) -> TezedgeStats {
        self.0.stats()
    }
}

impl<P> Acceptor<P> for TezedgeStateWrapper
where
    P: Proposal,
    TezedgeState: Acceptor<P>,
{
    #[inline]
    fn accept(&mut self, proposal: P) {
        // self.0.accept(dbg!(proposal))
        self.0.accept(proposal)
    }
}

impl GetRequests for TezedgeStateWrapper {
    type Request = TezedgeRequest;

    #[inline]
    fn get_requests(&self, buf: &mut Vec<Self::Request>) -> usize {
        self.0.get_requests(buf)
    }
}

impl From<TezedgeState> for TezedgeStateWrapper {
    #[inline]
    fn from(state: TezedgeState) -> Self {
        TezedgeStateWrapper(state)
    }
}
