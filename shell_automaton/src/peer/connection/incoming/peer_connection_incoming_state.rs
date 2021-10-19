use enum_kinds::EnumKind;
use serde::{Deserialize, Serialize};

use crate::peer::PeerToken;

use super::PeerConnectionIncomingError;

#[derive(EnumKind, Serialize, Deserialize, Debug, Clone)]
#[enum_kind(PeerConnectionIncomingStatePhase, derive(Serialize, Deserialize))]
pub enum PeerConnectionIncomingState {
    Pending {
        time: u64,
        token: PeerToken,
    },
    Error {
        time: u64,
        token: PeerToken,
        error: PeerConnectionIncomingError,
    },
    Success {
        time: u64,
        token: PeerToken,
    },
}

impl PeerConnectionIncomingState {
    pub fn token(&self) -> PeerToken {
        match self {
            Self::Pending { token, .. } => *token,
            Self::Error { token, .. } => *token,
            Self::Success { token, .. } => *token,
        }
    }

    pub fn time(&self) -> u64 {
        match self {
            Self::Pending { time, .. } | Self::Error { time, .. } | Self::Success { time, .. } => {
                *time
            }
        }
    }
}
