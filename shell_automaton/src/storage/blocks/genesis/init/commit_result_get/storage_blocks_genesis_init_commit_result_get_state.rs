// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use tezos_api::ffi::CommitGenesisResult;
use tezos_protocol_ipc_client::ProtocolServiceError;

use crate::protocol_runner::ProtocolRunnerToken;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum StorageBlocksGenesisInitCommitResultGetState {
    Init {},
    Pending { token: ProtocolRunnerToken },
    Error { error: ProtocolServiceError },
    Success { result: CommitGenesisResult },
}
