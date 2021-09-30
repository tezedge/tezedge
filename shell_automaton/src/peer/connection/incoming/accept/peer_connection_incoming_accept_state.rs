use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

use crate::peer::PeerToken;
use crate::service::mio_service::PeerConnectionIncomingAcceptError;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PeerConnectionIncomingAcceptState {
    Idle,
    Success {
        token: PeerToken,
        address: SocketAddr,
    },
    Error {
        error: PeerConnectionIncomingAcceptError,
    },
}
