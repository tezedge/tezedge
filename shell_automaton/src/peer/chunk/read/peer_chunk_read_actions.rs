use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

use super::PeerChunkReadError;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerChunkReadInitAction {
    pub address: SocketAddr,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerChunkReadPartAction {
    pub address: SocketAddr,
    pub bytes: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerChunkReadDecryptAction {
    pub address: SocketAddr,
    pub decrypted_bytes: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerChunkReadReadyAction {
    pub address: SocketAddr,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerChunkReadErrorAction {
    pub address: SocketAddr,
    pub error: PeerChunkReadError,
}
