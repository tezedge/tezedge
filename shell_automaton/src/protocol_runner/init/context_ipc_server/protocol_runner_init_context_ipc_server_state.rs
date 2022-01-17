// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use crate::protocol_runner::ProtocolRunnerToken;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ProtocolRunnerInitContextIpcServerState {
    Init {},
    Pending { token: ProtocolRunnerToken },
    Error { token: ProtocolRunnerToken },
    Success { token: Option<ProtocolRunnerToken> },
}
