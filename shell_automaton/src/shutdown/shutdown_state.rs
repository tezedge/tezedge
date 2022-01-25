// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct ShutdownPendingState {
    pub time: u64,
    pub protocol_runner_shutdown: bool,
}

impl ShutdownPendingState {
    pub fn is_complete(&self) -> bool {
        self.protocol_runner_shutdown
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ShutdownState {
    Idle,
    Init { time: u64 },
    Pending(ShutdownPendingState),
    Success { time: u64 },
}

impl ShutdownState {
    #[inline(always)]
    pub fn new() -> Self {
        Self::Idle
    }

    pub fn pending(time: u64) -> Self {
        Self::Pending(ShutdownPendingState {
            time,
            ..ShutdownPendingState::default()
        })
    }

    /// Check if pending shutdown is now complete.
    pub fn is_pending_complete(&self) -> bool {
        match self {
            Self::Pending(pending) => pending.is_complete(),
            _ => false,
        }
    }
}
