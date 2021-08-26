// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

//! This crate handles low level p2p communication.

use std::sync::Arc;

use crypto::hash::CryptoboxPublicKeyHash;
pub use tezedge_state::PeerAddress;
use tezos_identity::Identity;

#[derive(Debug, Clone)]
pub struct PeerId {
    pub address: PeerAddress,
    pub public_key_hash: CryptoboxPublicKeyHash,
}

pub mod network_channel;

pub use tezedge_state::ShellCompatibilityVersion;
