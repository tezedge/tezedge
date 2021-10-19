use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

use super::PeerConnectionIncomingStatePhase;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PeerConnectionIncomingError {
    Timeout(PeerConnectionIncomingStatePhase),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerConnectionIncomingErrorAction {
    pub address: SocketAddr,
    pub error: PeerConnectionIncomingError,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerConnectionIncomingSuccessAction {
    pub address: SocketAddr,
}
