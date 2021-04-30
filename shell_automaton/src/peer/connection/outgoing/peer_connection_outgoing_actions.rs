use serde::{Deserialize, Serialize};
use std::io;
use std::net::SocketAddr;

use crate::io_error_kind::IOErrorKind;
use crate::peer::PeerToken;

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
    pub error: IOErrorKind,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerConnectionOutgoingSuccessAction {
    pub address: SocketAddr,
}
