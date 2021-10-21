use serde::{Deserialize, Serialize};
use std::collections::btree_map::{BTreeMap, Entry as BTreeMapEntry};
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use crate::peer::{Peer, PeerStatus};

use super::check::timeouts::PeersCheckTimeoutsState;
use super::dns_lookup::PeersDnsLookupState;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PeerBlacklistState {
    /// Peer is temporarily graylisted.
    Graylisted { since: u64 },
}

impl PeerBlacklistState {
    pub fn timeout(&self, graylist_duration: Duration) -> Option<u64> {
        match self {
            Self::Graylisted { since } => Some(*since + graylist_duration.as_nanos() as u64),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeersState {
    list: BTreeMap<SocketAddr, Peer>,
    ip_blacklist: BTreeMap<IpAddr, PeerBlacklistState>,

    pub dns_lookup: Option<PeersDnsLookupState>,

    pub check_timeouts: PeersCheckTimeoutsState,
}

impl PeersState {
    pub fn new() -> Self {
        Self {
            list: BTreeMap::new(),
            ip_blacklist: BTreeMap::new(),

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

    /// Returns `Err` if peer's ip is blacklisted/graylisted.
    #[inline(always)]
    pub(super) fn entry<'a>(
        &'a mut self,
        address: SocketAddr,
    ) -> Result<BTreeMapEntry<'a, SocketAddr, Peer>, &'a PeerBlacklistState> {
        if let Some(blacklist_state) = self.ip_blacklist.get(&address.ip()) {
            return Err(blacklist_state);
        }
        Ok(self.list.entry(address))
    }

    #[inline(always)]
    pub(super) fn ip_blacklist_entry<'a>(
        &'a mut self,
        ip: IpAddr,
    ) -> BTreeMapEntry<'a, IpAddr, PeerBlacklistState> {
        self.ip_blacklist.entry(ip)
    }

    #[inline(always)]
    pub(super) fn remove_blacklisted_ip(&mut self, ip: &IpAddr) -> Option<PeerBlacklistState> {
        self.ip_blacklist.remove(ip)
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

    #[inline(always)]
    pub fn get_blacklisted_ip(&self, ip: &IpAddr) -> Option<&PeerBlacklistState> {
        self.ip_blacklist.get(ip)
    }

    #[inline(always)]
    pub fn blacklist_ip_iter<'a>(
        &'a self,
    ) -> impl 'a + Iterator<Item = (&'a IpAddr, &'a PeerBlacklistState)> {
        self.ip_blacklist.iter()
    }
}
