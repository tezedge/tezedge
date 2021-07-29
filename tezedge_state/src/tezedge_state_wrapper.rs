use std::fmt::Debug;
use std::time::Instant;

use tla_sm::GetRequests;
use tla_sm::{Acceptor, Proposal};

use crate::{DefaultEffects, PeerAddress, TezedgeConfig, TezedgeRequest, TezedgeState, TezedgeStats, ConnectedPeers};

#[derive(Debug, Clone)]
pub struct TezedgeStateWrapper<E = DefaultEffects>(TezedgeState<E>);

impl<E> TezedgeStateWrapper<E> {
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
    pub fn connected_peers(&self) -> &ConnectedPeers {
        self.0.connected_peers()
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

impl<E, P> Acceptor<P> for TezedgeStateWrapper<E>
where
    P: Proposal + Debug,
    TezedgeState<E>: Acceptor<P>,
{
    #[inline]
    fn accept(&mut self, proposal: P) {
        // self.0.accept(dbg!(proposal))
        self.0.accept(proposal)
    }
}

impl<E> GetRequests for TezedgeStateWrapper<E> {
    type Request = TezedgeRequest;

    #[inline]
    fn get_requests(&self, buf: &mut Vec<Self::Request>) -> usize {
        self.0.get_requests(buf)
    }
}

impl<E> From<TezedgeState<E>> for TezedgeStateWrapper<E> {
    #[inline]
    fn from(state: TezedgeState<E>) -> Self {
        TezedgeStateWrapper(state)
    }
}
