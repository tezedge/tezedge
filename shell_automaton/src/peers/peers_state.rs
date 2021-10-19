use redux_rs::ActionId;
use serde::{Deserialize, Serialize};
use std::collections::btree_map::{BTreeMap, Entry as BTreeMapEntry};
use std::net::SocketAddr;

use crate::peer::{Peer, PeerReadState, PeerStatus};

use super::check::timeouts::PeersCheckTimeoutsState;
use super::dns_lookup::PeersDnsLookupState;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeersState {
    pub list: BTreeMap<SocketAddr, Peer>,
    pub dns_lookup: Option<PeersDnsLookupState>,

    pub check_timeouts: PeersCheckTimeoutsState,
}

impl PeersState {
    pub fn new() -> Self {
        Self {
            list: BTreeMap::new(),
            dns_lookup: None,

            check_timeouts: PeersCheckTimeoutsState::new(),
        }
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.list.len()
    }

    #[inline(always)]
    pub fn get(&self, address: &SocketAddr) -> Option<&Peer> {
        self.list.get(address)
    }

    #[inline(always)]
    pub fn get_mut(&mut self, address: &SocketAddr) -> Option<&mut Peer> {
        self.list.get_mut(address)
    }

    #[inline(always)]
    pub(super) fn remove(&mut self, address: &SocketAddr) -> Option<Peer> {
        self.list.remove(address)
    }

    #[inline(always)]
    pub(super) fn entry<'a>(
        &'a mut self,
        address: SocketAddr,
    ) -> BTreeMapEntry<'a, SocketAddr, Peer> {
        self.list.entry(address)
    }

    #[inline(always)]
    pub fn iter<'a>(&'a self) -> impl 'a + Iterator<Item = (&'a SocketAddr, &'a Peer)> {
        self.list.iter()
    }

    #[inline(always)]
    pub fn iter_mut<'a>(&'a mut self) -> impl 'a + Iterator<Item = (&'a SocketAddr, &'a mut Peer)> {
        self.list.iter_mut()
    }

    /// Iterator over `Potential` peers.
    pub fn potential_iter<'a>(&'a self) -> impl 'a + Iterator<Item = SocketAddr> {
        self.iter()
            .filter(|(_, peer)| matches!(&peer.status, PeerStatus::Potential))
            .map(|(addr, _)| *addr)
    }

    /// Number of potential peers.
    pub fn potential_len(&self) -> usize {
        self.potential_iter().count()
    }

    /// Number of peers that we have established tcp connection with.
    pub fn connected_len(&self) -> usize {
        self.iter().filter(|(_, peer)| peer.is_connected()).count()
    }

    /// Iterator of peers that can read from their sockets. Such peer can be in
    /// readable state, or in the state when it exceeded its read quota, but
    /// enough time passed since then and the quota should be restored.
    pub fn readable(
        &self,
        now: ActionId,
        quota_restore_duration_millis: u128,
    ) -> impl Iterator<Item = (&SocketAddr, &Peer)> {
        self.iter()
            .filter_map(move |(address, peer)| match peer.read_state {
                PeerReadState::Readable { .. } => Some((address, peer)),
                PeerReadState::OutOfQuota { timestamp }
                    if now.duration_since(timestamp).as_millis()
                        >= quota_restore_duration_millis =>
                {
                    Some((address, peer))
                }
                _ => None,
            })
    }

    /// Iterator of peers that can write to their sockets. Such peer can be in
    /// writable state, or in the state when it exceeded its write quota, but
    /// enough time passed since then and the quota should be restored.
    pub fn writable(
        &self,
        now: ActionId,
        quota_restore_duration_millis: u128,
    ) -> impl Iterator<Item = (&SocketAddr, &Peer)> {
        self.iter()
            .filter_map(move |(address, peer)| match peer.read_state {
                PeerReadState::Readable { .. } => Some((address, peer)),
                PeerReadState::OutOfQuota { timestamp }
                    if now.duration_since(timestamp).as_millis()
                        >= quota_restore_duration_millis =>
                {
                    Some((address, peer))
                }
                _ => None,
            })
    }
}
