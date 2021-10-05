use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

use super::PeerBinaryMessageReadError;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerBinaryMessageReadInitAction {
    pub address: SocketAddr,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerBinaryMessageReadChunkReadyAction {
    pub address: SocketAddr,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerBinaryMessageReadSizeReadyAction {
    pub address: SocketAddr,
    pub size: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerBinaryMessageReadReadyAction {
    pub address: SocketAddr,
    pub message: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerBinaryMessageReadErrorAction {
    pub address: SocketAddr,
    pub error: PeerBinaryMessageReadError,
}
