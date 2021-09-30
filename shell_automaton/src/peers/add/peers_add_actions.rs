use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

use crate::peer::PeerToken;

/// Add incoming peer to peers.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeersAddIncomingPeerAction {
    pub token: PeerToken,
    pub address: SocketAddr,
}
