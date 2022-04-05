// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
pub enum PeerRequestsPotentialPeersGetError {
    Timeout,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PeerRequestsPotentialPeersGetState {
    Idle {
        time: u64,
    },
    Init {
        time: u64,
    },
    Pending {
        time: u64,
    },
    Error {
        time: u64,
        error: PeerRequestsPotentialPeersGetError,
    },
    Success {
        time: u64,
        result: Vec<SocketAddr>,
    },
}

impl PeerRequestsPotentialPeersGetState {
    pub fn time(&self) -> u64 {
        match self {
            Self::Idle { time } => *time,
            Self::Init { time } => *time,
            Self::Pending { time } => *time,
            Self::Error { time, .. } => *time,
            Self::Success { time, .. } => *time,
        }
    }

    pub fn is_init(&self) -> bool {
        matches!(self, Self::Init { .. })
    }

    pub fn is_pending(&self) -> bool {
        matches!(self, Self::Pending { .. })
    }

    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success { .. })
    }
}

impl Default for PeerRequestsPotentialPeersGetState {
    fn default() -> Self {
        Self::Idle { time: 0 }
    }
}
