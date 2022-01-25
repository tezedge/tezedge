// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use tezos_protocol_ipc_client::ProtocolRunnerError;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ProtocolRunnerSpawnServerState {
    Init,
    Pending {},
    Error { error: ProtocolRunnerError },
    Success {},
}
