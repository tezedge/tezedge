// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::btree_map::{BTreeMap, Entry as BTreeMapEntry};
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crypto::hash::BlockHash;

use crate::peer::{Peer, PeerHandshaked, PeerStatus};

use super::check::timeouts::PeersCheckTimeoutsState;
use super::dns_lookup::PeersDnsLookupState;

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
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
    pub list: BTreeMap<SocketAddr, Peer>,
    ip_blacklist: BTreeMap<IpAddr, PeerBlacklistState>,

    pub dns_lookup: Option<PeersDnsLookupState>,

    pub check_timeouts: PeersCheckTimeoutsState,

    // TODO(zura): implement p2p peer requests to better track each request.
    /// Maps BlockHash to time request was initiated.
    pub pending_block_header_requests: BTreeMap<BlockHash, u64>,
}

impl PeersState {
    pub fn new() -> Self {
        Self {
            list: BTreeMap::new(),
            ip_blacklist: BTreeMap::new(),

            dns_lookup: None,

            check_timeouts: PeersCheckTimeoutsState::new(),

            pending_block_header_requests: BTreeMap::new(),
        }
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.list.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline(always)]
    pub fn get(&self, address: &SocketAddr) -> Option<&Peer> {
        self.list.get(address)
    }

    #[inline(always)]
    pub fn get_handshaked(&self, address: &SocketAddr) -> Option<&PeerHandshaked> {
        self.list
            .get(address)
            .and_then(|p| p.status.as_handshaked())
    }

    #[inline(always)]
    pub fn get_mut(&mut self, address: &SocketAddr) -> Option<&mut Peer> {
        self.list.get_mut(address)
    }

    #[inline(always)]
    pub fn get_handshaked_mut(&mut self, address: &SocketAddr) -> Option<&mut PeerHandshaked> {
        self.list
            .get_mut(address)
            .and_then(|p| p.status.as_handshaked_mut())
    }

    #[inline(always)]
    pub(super) fn remove(&mut self, address: &SocketAddr) -> Option<Peer> {
        self.list.remove(address)
    }

    /// Returns `Err` if peer's ip is blacklisted/graylisted.
    #[inline(always)]
    pub(super) fn entry(
        &mut self,
        address: SocketAddr,
    ) -> Result<BTreeMapEntry<SocketAddr, Peer>, &PeerBlacklistState> {
        if let Some(blacklist_state) = self.ip_blacklist.get(&address.ip()) {
            return Err(blacklist_state);
        }
        Ok(self.list.entry(address))
    }

    #[inline(always)]
    pub(super) fn ip_blacklist_entry(
        &mut self,
        ip: IpAddr,
    ) -> BTreeMapEntry<IpAddr, PeerBlacklistState> {
        self.ip_blacklist.entry(ip)
    }

    #[inline(always)]
    pub(super) fn remove_blacklisted_ip(&mut self, ip: &IpAddr) -> Option<PeerBlacklistState> {
        self.ip_blacklist.remove(ip)
    }

    #[inline(always)]
    pub fn iter(&self) -> impl Iterator<Item = (&SocketAddr, &Peer)> {
        self.list.iter()
    }

    pub fn iter_addr(&self) -> impl Iterator<Item = &SocketAddr> {
        self.list.keys()
    }

    pub fn iter_handshaked(&self) -> impl Iterator<Item = (&SocketAddr, &Peer)> {
        self.list.iter().filter(|(_, p)| p.is_handshaked())
    }

    #[inline(always)]
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&SocketAddr, &mut Peer)> {
        self.list.iter_mut()
    }

    /// Iterator over `Potential` peers.
    pub fn potential_iter(&self) -> impl Iterator<Item = SocketAddr> + '_ {
        self.iter()
            .filter(|(_, peer)| matches!(&peer.status, PeerStatus::Potential))
            .map(|(addr, _)| *addr)
    }

    /// Number of potential peers.
    pub fn potential_len(&self) -> usize {
        self.potential_iter().count()
    }

    /// Iterator over `Connected` peers.
    pub fn connected_iter(&self) -> impl Iterator<Item = (&SocketAddr, &Peer)> {
        self.list.iter().filter(|(_, peer)| peer.is_connected())
    }

    /// Number of peers that we have established tcp connection with.
    pub fn connected_len(&self) -> usize {
        self.connected_iter().count()
    }

    #[inline(always)]
    pub fn is_blacklisted(&self, ip: &IpAddr) -> bool {
        self.get_blacklisted_ip(ip).is_some()
    }

    /// Iterator over handshaked peers.
    pub fn handshaked_iter(&self) -> impl Iterator<Item = (SocketAddr, &PeerHandshaked)> {
        self.iter()
            .filter_map(|(addr, peer)| peer.status.as_handshaked().map(|p| (*addr, p)))
    }

    /// Number of peers that are handshaked
    pub fn handshaked_len(&self) -> usize {
        self.handshaked_iter().count()
    }

    #[inline(always)]
    pub fn get_blacklisted_ip(&self, ip: &IpAddr) -> Option<&PeerBlacklistState> {
        self.ip_blacklist.get(ip)
    }

    #[inline(always)]
    pub fn blacklist_ip_iter(&self) -> impl Iterator<Item = (&IpAddr, &PeerBlacklistState)> {
        self.ip_blacklist.iter()
    }
}

impl Default for PeersState {
    fn default() -> Self {
        Self::new()
    }
}
