// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

//! This crate handles low level p2p communication.

use std::net::SocketAddr;

use crypto::hash::CryptoboxPublicKeyHash;

use crate::p2p::peer::PeerRef;

pub mod p2p;

/// Unificated Peer identification
#[derive(Clone, Debug)]
pub struct PeerId {
    /// Peer actor ref
    pub peer_ref: PeerRef,
    /// Peer public key hash (hash of PublicKey)
    pub peer_public_key_hash: CryptoboxPublicKeyHash,
    pub peer_id_marker: String,
    /// Peer address
    pub peer_address: SocketAddr,
}

impl PeerId {
    pub fn new(peer_ref: PeerRef, peer_public_key_hash: CryptoboxPublicKeyHash, peer_id_marker: String, peer_address: SocketAddr) -> Self {
        Self {
            peer_ref,
            peer_public_key_hash,
            peer_id_marker,
            peer_address,
        }
    }
}