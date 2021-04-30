use serde::{Deserialize, Serialize};
use std::io;
use std::net::SocketAddr;

use crate::io_error_kind::IOErrorKind;
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
