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
