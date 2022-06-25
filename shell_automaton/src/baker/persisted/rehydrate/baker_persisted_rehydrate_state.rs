// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use crate::baker::persisted::PersistedState;
use crate::request::RequestId;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum BakerPersistedRehydrateState {
    Idle {
        time: u64,
    },
    Pending {
        time: u64,
        req_id: RequestId,
    },
    Success {
        time: u64,
        result: PersistedState,
    },
    Rehydrated {
        time: u64,
        rehydrated_state: PersistedState,
        current_state: PersistedState,
        last_persisted_counter: u64,
    },
}

impl BakerPersistedRehydrateState {
    pub fn is_rehydrated(&self) -> bool {
        match self {
            Self::Rehydrated { .. } => true,
            _ => false,
        }
    }
}
