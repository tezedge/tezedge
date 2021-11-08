// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Serialize, Deserialize};

use tezos_api::ffi::{InitProtocolContextResult, PrevalidatorWrapper};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ProtocolAction {
    Error(String),
    InitProtocolDone(InitProtocolContextResult),
    PrevalidatorReady(PrevalidatorWrapper),
}
