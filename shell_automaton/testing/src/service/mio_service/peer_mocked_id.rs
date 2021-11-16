// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::net::SocketAddr;

use shell_automaton::peer::PeerToken;

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub struct MioPeerMockedId {
    id: usize,
}

impl MioPeerMockedId {
    /// Create new unchecked peer id. Caller must ensure that the peer
    /// with such id exists.
    pub fn new_unchecked(id: usize) -> Self {
        Self { id }
    }

    #[inline]
    pub fn index(&self) -> usize {
        self.id
    }

    pub fn from_ipv4(addr: &SocketAddr) -> Self {
        match addr {
            SocketAddr::V4(addr) => {
                let [a, b, c, d] = addr.ip().octets();
                let (a, b, c, d) = (a as usize, b as usize, c as usize, d as usize);
                Self::new_unchecked(a * 256 * 256 * 256 + b * 256 * 256 + c * 256 + d)
            }
            SocketAddr::V6(_) => panic!("expected ipv4, found ipv6"),
        }
    }

    pub fn to_ipv4(&self) -> SocketAddr {
        let id = self.id;

        let ip = [
            ((id / 256 / 256 / 256) % 256) as u8,
            ((id / 256 / 256) % 256) as u8,
            ((id / 256) % 256) as u8,
            (id % 256) as u8,
        ];
        (ip, 12345).into()
    }

    pub fn from_token(token: &PeerToken) -> Self {
        Self { id: token.index() }
    }

    pub fn to_token(&self) -> PeerToken {
        PeerToken::new_unchecked(self.id)
    }
}

impl From<PeerToken> for MioPeerMockedId {
    fn from(token: PeerToken) -> Self {
        Self { id: token.into() }
    }
}
