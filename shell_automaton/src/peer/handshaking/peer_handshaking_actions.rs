use serde::{Deserialize, Serialize};
use std::io;
use std::net::SocketAddr;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerHandshakingInitAction {
    pub address: SocketAddr,
}
