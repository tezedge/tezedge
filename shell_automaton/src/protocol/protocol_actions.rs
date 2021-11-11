// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Serialize, Deserialize};

use tezos_api::ffi::{InitProtocolContextResult, PrevalidatorWrapper};

use crate::{EnablingCondition, State};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ProtocolAction {
    Error(String),
    InitProtocolDone(InitProtocolContextResult),
    PrevalidatorReady(PrevalidatorWrapper),
    PrevalidatorForMempoolReady(PrevalidatorWrapper),
}

impl EnablingCondition<State> for ProtocolAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}