// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use derive_more::From;
use serde::{Deserialize, Serialize};

use crypto::hash::ContextHash;

use super::init::ProtocolRunnerInitState;
use super::spawn_server::ProtocolRunnerSpawnServerState;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProtocolRunnerReadyState {
    pub genesis_commit_hash: Option<ContextHash>,
}

#[derive(From, Serialize, Deserialize, Debug, Clone)]
pub enum ProtocolRunnerState {
    Idle,

    #[from]
    SpawnServer(ProtocolRunnerSpawnServerState),

    #[from]
    Init(ProtocolRunnerInitState),

    #[from]
    Ready(ProtocolRunnerReadyState),

    ShutdownPending,
    ShutdownSuccess,
}
