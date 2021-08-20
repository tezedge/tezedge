// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;
use std::fmt::{self, Display};

use crate::TezedgeState;

#[derive(Debug, Clone, Copy)]
enum PeerContainer {
    Potential,
    Pending,
    Connected,
    Blacklisted,
}

impl fmt::Display for PeerContainer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Potential => "potential_peers",
                Self::Pending => "pending_peers",
                Self::Connected => "connected_peers",
                Self::Blacklisted => "blacklisted_peers",
            }
        )
    }
}

#[inline]
fn assert_peer_counts(state: &TezedgeState) {
    assert!(state.potential_peers.len() <= state.config.max_potential_peers);
    assert!(state.pending_peers.len() <= state.config.max_pending_peers);
    assert!(state.connected_peers.len() <= state.config.max_connected_peers);
}

#[inline]
fn assert_no_overlapping_peers(state: &TezedgeState) {
    fn panic_overlap<A: Display>(addr: A, first: PeerContainer, second: PeerContainer) {
        panic!(
            "Found an overlap between peer containers! Address: {}, Containers: ({}, {})",
            addr, first, second
        );
    }

    let mut all_peers = BTreeMap::new();

    let container_type = PeerContainer::Potential;
    for peer in state.potential_peers.iter() {
        all_peers.insert(*peer, container_type);
    }

    let container_type = PeerContainer::Pending;
    for (_, peer) in state.pending_peers.iter() {
        if let Some(addr) = peer.listener_address() {
            if let Some(existing) = all_peers.insert(addr, container_type) {
                panic_overlap(addr, existing, container_type);
            }
        }
    }

    let container_type = PeerContainer::Connected;
    for peer in state.connected_peers.iter() {
        if let Some(existing) = all_peers.insert(peer.listener_address(), container_type) {
            panic_overlap(peer.listener_address(), existing, container_type);
        }
    }

    for (addr, container) in all_peers.into_iter() {
        if state.is_address_blacklisted(&addr.into()) {
            panic_overlap(addr, container, PeerContainer::Blacklisted);
        }
    }
}

impl TezedgeState {
    pub fn assert_state(&self) {
        assert_peer_counts(self);
        assert_no_overlapping_peers(self);
    }
}
