use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

/// Add multiple peers as potential peers.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeersAddMultiAction {
    pub addresses: Vec<SocketAddr>,
}
