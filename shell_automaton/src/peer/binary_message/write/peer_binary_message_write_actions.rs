use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

use super::PeerBinaryMessageWriteError;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerBinaryMessageWriteSetContentAction {
    pub address: SocketAddr,
    pub message: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerBinaryMessageWriteNextChunkAction {
    pub address: SocketAddr,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerBinaryMessageWriteReadyAction {
    pub address: SocketAddr,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerBinaryMessageWriteErrorAction {
    pub address: SocketAddr,
    pub error: PeerBinaryMessageWriteError,
}
