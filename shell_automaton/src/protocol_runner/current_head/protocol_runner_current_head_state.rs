// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crypto::hash::ContextHash;
use serde::{Deserialize, Serialize};
use tezos_protocol_ipc_client::ProtocolRunnerError;

use crate::protocol_runner::ProtocolRunnerToken;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ProtocolRunnerCurrentHeadState {
    Init {
        genesis_commit_hash: Option<ContextHash>,
    },
    Pending {
        genesis_commit_hash: Option<ContextHash>,
        token: ProtocolRunnerToken,
    },
    Error {
        genesis_commit_hash: Option<ContextHash>,
        token: ProtocolRunnerToken,
        error: ProtocolRunnerError,
    },
    Success {
        genesis_commit_hash: Option<ContextHash>,
        latest_context_hashes: Vec<ContextHash>,
        token: ProtocolRunnerToken,
    },
}
