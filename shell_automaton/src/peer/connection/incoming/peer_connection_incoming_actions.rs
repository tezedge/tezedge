use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerConnectionIncomingSuccessAction {
    pub address: SocketAddr,
}
