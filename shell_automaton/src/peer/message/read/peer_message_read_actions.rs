use std::net::SocketAddr;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use tezos_messages::p2p::encoding::peer::PeerMessageResponse;

use super::PeerMessageReadError;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerMessageReadInitAction {
    pub address: SocketAddr,
}

/// PeerMessage has been read/received successfuly.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerMessageReadErrorAction {
    pub address: SocketAddr,
    pub error: PeerMessageReadError,
}

/// PeerMessage has been read/received successfuly.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerMessageReadSuccessAction {
    pub address: SocketAddr,
    pub message: Arc<PeerMessageResponse>,
}
