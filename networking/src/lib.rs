// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

pub mod network_channel;

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

use crypto::hash::CryptoboxPublicKeyHash;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerId {
    pub address: SocketAddr,
    pub public_key_hash: CryptoboxPublicKeyHash,
}

impl Eq for PeerId {}
impl PartialEq for PeerId {
    fn eq(&self, other: &Self) -> bool {
        self.address == other.address && self.public_key_hash == other.public_key_hash
    }
}
impl std::hash::Hash for PeerId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.address.hash(state);
        self.public_key_hash.hash(state);
    }
}
