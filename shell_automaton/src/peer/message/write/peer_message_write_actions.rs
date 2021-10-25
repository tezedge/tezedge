use std::net::SocketAddr;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use tezos_messages::p2p::encoding::peer::PeerMessageResponse;

use super::PeerMessageWriteError;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerMessageWriteNextAction {
    pub address: SocketAddr,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerMessageWriteInitAction {
    pub address: SocketAddr,
    pub message: Arc<PeerMessageResponse>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerMessageWriteErrorAction {
    pub address: SocketAddr,
    pub error: PeerMessageWriteError,
}

/// PeerMessage has been read/received successfuly.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerMessageWriteSuccessAction {
    pub address: SocketAddr,
}
