// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use crate::request::RequestId;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum BakerPersistedPersistState {
    Idle {
        time: u64,
    },
    Pending {
        time: u64,
        req_id: RequestId,
        counter: u64,
    },
    Success {
        time: u64,
        counter: u64,
    },
}

impl BakerPersistedPersistState {
    pub fn counter(&self) -> u64 {
        match self {
            Self::Idle { .. } => 0,
            Self::Pending { counter, .. } | Self::Success { counter, .. } => *counter,
        }
    }
}
