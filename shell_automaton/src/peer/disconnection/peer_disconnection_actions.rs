use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

/// Disconnect connected peer.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerDisconnectAction {
    pub address: SocketAddr,
}

/// Connected peer disconnected.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerDisconnectedAction {
    pub address: SocketAddr,
}
