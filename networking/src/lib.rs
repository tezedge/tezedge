// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;

use crypto::hash::BlockHash;
use crypto::hash::CryptoboxPublicKeyHash;

pub mod network_channel;

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

/// Event is fired, when some batch was finished, so next can go
#[derive(Clone, Debug)]
pub struct ApplyBlockDone {
    pub last_applied: Arc<BlockHash>,
}

/// Event is fired, when some batch was not applied and error occured
#[derive(Clone, Debug)]
pub struct ApplyBlockFailed {
    pub failed_block: Arc<BlockHash>,
}
