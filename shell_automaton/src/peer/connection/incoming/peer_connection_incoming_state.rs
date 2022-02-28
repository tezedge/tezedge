// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use enum_kinds::EnumKind;
use serde::{Deserialize, Serialize};

use crate::peer::PeerToken;

use super::PeerConnectionIncomingError;

#[cfg(feature = "fuzzing")]
use crate::fuzzing::net::PeerTokenMutator;

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(EnumKind, Serialize, Deserialize, Debug, Clone)]
#[enum_kind(
    PeerConnectionIncomingStatePhase,
    derive(Serialize, Deserialize),
    cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))
)]
pub enum PeerConnectionIncomingState {
    Pending {
        time: u64,
        #[cfg_attr(feature = "fuzzing", field_mutator(PeerTokenMutator))]
        token: PeerToken,
    },
    Error {
        time: u64,
        #[cfg_attr(feature = "fuzzing", field_mutator(PeerTokenMutator))]
        token: PeerToken,
        error: PeerConnectionIncomingError,
    },
    Success {
        time: u64,
        #[cfg_attr(feature = "fuzzing", field_mutator(PeerTokenMutator))]
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
