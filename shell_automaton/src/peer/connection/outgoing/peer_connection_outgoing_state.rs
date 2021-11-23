// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use enum_kinds::EnumKind;
use serde::{Deserialize, Serialize};

use crate::peer::PeerToken;

use super::PeerConnectionOutgoingError;

#[cfg(feature = "fuzzing")]
use crate::fuzzing::net::PeerTokenMutator;
#[cfg(feature = "fuzzing")]
use fuzzcheck::mutators::option::OptionMutator;

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(EnumKind, Serialize, Deserialize, Debug, Clone)]
#[enum_kind(
    PeerConnectionOutgoingStatePhase,
    derive(Serialize, Deserialize),
    cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))
)]
pub enum PeerConnectionOutgoingState {
    Idle {
        time: u64,
    },
    Pending {
        time: u64,
        #[cfg_attr(feature = "fuzzing", field_mutator(PeerTokenMutator))]
        token: PeerToken,
    },
    Error {
        time: u64,
        #[cfg_attr(feature = "fuzzing", field_mutator(OptionMutator<PeerToken, PeerTokenMutator>))]
        token: Option<PeerToken>,
        error: PeerConnectionOutgoingError,
    },
    Success {
        time: u64,
        #[cfg_attr(feature = "fuzzing", field_mutator(PeerTokenMutator))]
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
