// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::net::SocketAddr;

use tezedge_state::PeerAddress;

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub struct FakePeerId {
    id: usize,
}

impl FakePeerId {
    /// Create new unchecked peer id. Caller must ensure that the peer
    /// with such id exists.
    pub fn new_unchecked(id: usize) -> Self {
        Self { id }
    }

    #[inline]
    pub fn index(&self) -> usize {
        self.id
    }
}

impl From<FakePeerId> for PeerAddress {
    fn from(id: FakePeerId) -> Self {
        PeerAddress::ipv4_from_index(id.id as u64)
    }
}

impl From<FakePeerId> for SocketAddr {
    fn from(id: FakePeerId) -> Self {
        PeerAddress::from(id).into()
    }
}

impl From<PeerAddress> for FakePeerId {
    fn from(addr: PeerAddress) -> Self {
        Self {
            id: addr.to_index() as usize,
        }
    }
}

impl From<&PeerAddress> for FakePeerId {
    fn from(addr: &PeerAddress) -> Self {
        Self {
            id: addr.to_index() as usize,
        }
    }
}
