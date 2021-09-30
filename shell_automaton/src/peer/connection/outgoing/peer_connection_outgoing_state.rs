use crate::io_error_kind::IOErrorKind;
use crate::peer::PeerToken;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PeerConnectionOutgoingState {
    Idle,
    Pending { token: PeerToken },
    Error { error: IOErrorKind },
    Success { token: PeerToken },
}

impl PeerConnectionOutgoingState {
    pub fn token(&self) -> Option<PeerToken> {
        match self {
            Self::Idle => None,
            Self::Pending { token } => Some(*token),
            Self::Error { .. } => None,
            Self::Success { token } => Some(*token),
        }
    }
}
