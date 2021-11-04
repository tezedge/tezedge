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
    /// Peer actor ref
    pub peer_ref: PeerRef,
}

impl Eq for PeerId {}
impl PartialEq for PeerId {
    fn eq(&self, other: &Self) -> bool {
        self.peer_ref == other.peer_ref
    }
}
impl std::hash::Hash for PeerId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        use tezedge_actor_system::actors::ActorReference;
        self.peer_ref.uri().hash(state);
    }
}
