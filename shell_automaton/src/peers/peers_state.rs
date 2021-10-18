use serde::{Deserialize, Serialize};
use std::collections::btree_map::{BTreeMap, Entry as BTreeMapEntry};
use std::net::SocketAddr;

use crate::peer::Peer;

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
}
