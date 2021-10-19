use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

use crate::io_error_kind::IOErrorKind;
use crate::peer::PeerToken;

use super::PeerConnectionOutgoingStatePhase;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PeerConnectionOutgoingError {
    IO(IOErrorKind),
    Timeout(PeerConnectionOutgoingStatePhase),
}

impl From<std::io::Error> for PeerConnectionOutgoingError {
    fn from(error: std::io::Error) -> Self {
        Self::IO(error.kind().into())
    }
}

/// Initialize outgoing connection to a random potential peer.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerConnectionOutgoingRandomInitAction {}

/// Initialize outgoing connection to potential peer.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerConnectionOutgoingInitAction {
    pub address: SocketAddr,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerConnectionOutgoingPendingAction {
    pub address: SocketAddr,
    pub token: PeerToken,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerConnectionOutgoingErrorAction {
    pub address: SocketAddr,
    pub error: PeerConnectionOutgoingError,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerConnectionOutgoingSuccessAction {
    pub address: SocketAddr,
}
