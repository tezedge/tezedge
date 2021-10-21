use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

use derive_more::From;

use crate::state::PeerDispatchRecursionLimitExceededError;

/// Try writing next part/message to the peer.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerTryWriteAction {
    pub address: SocketAddr,
}

/// Try reading from peer.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerTryReadAction {
    pub address: SocketAddr,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerReadWouldBlockAction {
    pub address: SocketAddr,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerWriteWouldBlockAction {
    pub address: SocketAddr,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerErrorAction {
    pub address: SocketAddr,
    pub error: PeerError,
}

#[derive(Serialize, Deserialize, Debug, Clone, From)]
pub enum PeerError {
    Recursion(PeerDispatchRecursionLimitExceededError),
}
