use derive_more::From;
use serde::{Deserialize, Serialize};
use std::io;

use crate::io_error_kind::IOErrorKind;
use crate::peer::connection::incoming::PeerConnectionIncomingState;
use crate::peer::connection::outgoing::PeerConnectionOutgoingState;
use crate::peer::PeerToken;

#[derive(From, Serialize, Deserialize, Debug, Clone)]
pub enum PeerConnectionState {
    Outgoing(PeerConnectionOutgoingState),
    Incoming(PeerConnectionIncomingState),
}

impl PeerConnectionState {
    pub fn token(&self) -> Option<PeerToken> {
        match self {
            Self::Outgoing(s) => s.token(),
            Self::Incoming(s) => s.token(),
        }
    }
}
