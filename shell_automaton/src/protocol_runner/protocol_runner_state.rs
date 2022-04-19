// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use derive_more::From;
use serde::{Deserialize, Serialize};

use crypto::hash::ContextHash;

use super::current_head::ProtocolRunnerCurrentHeadState;
use super::init::ProtocolRunnerInitState;
use super::spawn_server::ProtocolRunnerSpawnServerState;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProtocolRunnerReadyState {
    pub genesis_commit_hash: Option<ContextHash>,
    pub latest_context_hashes: Vec<ContextHash>,
}

#[derive(From, Serialize, Deserialize, Debug, Clone)]
pub enum ProtocolRunnerState {
    /// Protocol runner not started yet
    Idle,

    /// Spawning protocol runner process
    #[from]
    SpawnServer(ProtocolRunnerSpawnServerState),

    /// Initializing protocol runner
    #[from]
    Init(ProtocolRunnerInitState),

    /// Load context's current head
    #[from]
    GetCurrentHead(ProtocolRunnerCurrentHeadState),

    /// Protocol runner intialized and ready
    #[from]
    Ready(ProtocolRunnerReadyState),

    /// Shutdown issued and in progress
    ShutdownPending,

    /// Shutdown successfully completed
    ShutdownSuccess,
}

impl ProtocolRunnerState {
    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready(_))
    }
}
