use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

use crate::peer::PeerToken;
use crate::service::mio_service::PeerConnectionIncomingAcceptError;

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum PeerConnectionIncomingRejectedReason {
    PeersConnectedMaxBoundReached,
}

/// Accept incoming peer connection.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerConnectionIncomingAcceptAction {}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerConnectionIncomingAcceptErrorAction {
    pub error: PeerConnectionIncomingAcceptError,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerConnectionIncomingRejectedAction {
    pub token: PeerToken,
    pub address: SocketAddr,
    pub reason: PeerConnectionIncomingRejectedReason,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerConnectionIncomingAcceptSuccessAction {
    pub token: PeerToken,
    pub address: SocketAddr,
}
