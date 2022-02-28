// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use tezos_api::ffi::{InitProtocolContextResult, PrevalidatorWrapper, ValidateOperationResponse};

use crate::{EnablingCondition, State};

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ProtocolAction {
    Error(String),
    InitProtocolDone(InitProtocolContextResult),
    PrevalidatorReady(PrevalidatorWrapper),
    PrevalidatorForMempoolReady(PrevalidatorWrapper),
    OperationValidated(ValidateOperationResponse),
}

impl EnablingCondition<State> for ProtocolAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}
