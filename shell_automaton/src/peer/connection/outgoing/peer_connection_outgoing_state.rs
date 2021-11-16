// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use enum_kinds::EnumKind;
use serde::{Deserialize, Serialize};

use crate::peer::PeerToken;

use super::PeerConnectionOutgoingError;

#[derive(EnumKind, Serialize, Deserialize, Debug, Clone)]
#[enum_kind(PeerConnectionOutgoingStatePhase, derive(Serialize, Deserialize))]
pub enum PeerConnectionOutgoingState {
    Idle {
        time: u64,
    },
    Pending {
        time: u64,
        token: PeerToken,
    },
    Error {
        time: u64,
        token: Option<PeerToken>,
        error: PeerConnectionOutgoingError,
    },
    Success {
        time: u64,
        token: PeerToken,
    },
}

impl PeerConnectionOutgoingState {
    pub fn token(&self) -> Option<PeerToken> {
        match self {
            Self::Idle { .. } => None,
            Self::Pending { token, .. } => Some(*token),
            Self::Error { token, .. } => token.clone(),
            Self::Success { token, .. } => Some(*token),
        }
    }

    pub fn time(&self) -> u64 {
        match self {
            Self::Idle { time, .. }
            | Self::Pending { time, .. }
            | Self::Error { time, .. }
            | Self::Success { time, .. } => *time,
        }
    }
}
