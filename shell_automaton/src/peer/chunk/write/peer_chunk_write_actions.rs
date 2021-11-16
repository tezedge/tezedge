// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

use tezos_messages::p2p::binary_message::BinaryChunk;

use super::PeerChunkWriteError;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerChunkWriteSetContentAction {
    pub address: SocketAddr,
    pub content: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerChunkWriteEncryptContentAction {
    pub address: SocketAddr,
    pub encrypted_content: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerChunkWriteCreateChunkAction {
    pub address: SocketAddr,
    pub chunk: BinaryChunk,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerChunkWritePartAction {
    pub address: SocketAddr,
    pub written: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerChunkWriteErrorAction {
    pub address: SocketAddr,
    pub error: PeerChunkWriteError,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerChunkWriteReadyAction {
    pub address: SocketAddr,
}
