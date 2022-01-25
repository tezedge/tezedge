// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use tezos_api::ffi::InitProtocolContextResult;

use crate::protocol_runner::ProtocolRunnerToken;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ProtocolRunnerInitContextState {
    Init {
        apply_genesis: bool,
    },
    Pending {
        token: ProtocolRunnerToken,
        apply_genesis: bool,
    },
    Error {
        token: Option<ProtocolRunnerToken>,
        apply_genesis: bool,
    },
    Success {
        token: ProtocolRunnerToken,
        apply_genesis: bool,
        result: InitProtocolContextResult,
    },
}

impl ProtocolRunnerInitContextState {
    pub fn apply_genesis(&self) -> bool {
        match self {
            Self::Init { apply_genesis, .. } => *apply_genesis,
            Self::Pending { apply_genesis, .. } => *apply_genesis,
            Self::Error { apply_genesis, .. } => *apply_genesis,
            Self::Success { apply_genesis, .. } => *apply_genesis,
        }
    }
}
