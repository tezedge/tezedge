use serde::{Deserialize, Serialize};

use crate::peer::PeerToken;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PeerConnectionIncomingState {
    Pending { token: PeerToken },
    Success { token: PeerToken },
}

impl PeerConnectionIncomingState {
    pub fn token(&self) -> Option<PeerToken> {
        match self {
            Self::Pending { token } => Some(*token),
            Self::Success { token } => Some(*token),
        }
    }
}
