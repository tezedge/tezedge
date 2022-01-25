// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crypto::hash::ContextHash;
use derive_more::From;
use serde::{Deserialize, Serialize};

use tezos_api::ffi::InitProtocolContextResult;

use super::context::ProtocolRunnerInitContextState;
use super::context_ipc_server::ProtocolRunnerInitContextIpcServerState;
use super::runtime::ProtocolRunnerInitRuntimeState;

#[derive(From, Serialize, Deserialize, Debug, Clone)]
pub enum ProtocolRunnerInitState {
    Init {},

    #[from]
    Runtime(ProtocolRunnerInitRuntimeState),

    /// Check if we need to commit genesis.
    CheckGenesisApplied {},
    CheckGenesisAppliedSuccess {
        is_applied: bool,
    },

    #[from]
    Context(ProtocolRunnerInitContextState),

    #[from]
    ContextIpcServer(
        (
            InitProtocolContextResult,
            ProtocolRunnerInitContextIpcServerState,
        ),
    ),

    Success {
        genesis_commit_hash: Option<ContextHash>,
    },
}
