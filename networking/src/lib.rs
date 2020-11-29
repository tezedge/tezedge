// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

//! This crate handles low level p2p communication.

use std::net::SocketAddr;

use crypto::hash::{CryptoboxPublicKeyHash, HashType};

use crate::p2p::peer::PeerRef;

pub mod p2p;

pub type PeerPublicKey = CryptoboxPublicKeyHash;

/// Unificated Peer identification
#[derive(Clone, Debug)]
pub struct PeerId {
    /// Peer actor ref
    pub peer_ref: PeerRef,
    /// Peer public key
    pub peer_public_key: PeerPublicKey,
    pub peer_id_marker: String,
    /// Peer address
    pub peer_address: SocketAddr,
}

impl PeerId {
    pub fn new(peer_ref: PeerRef, peer_public_key: PeerPublicKey, peer_address: SocketAddr) -> Self {
        let peer_id_marker = HashType::CryptoboxPublicKeyHash.bytes_to_string(&peer_public_key);
        Self {
            peer_ref,
            peer_public_key,
            peer_id_marker,
            peer_address,
        }
    }
}