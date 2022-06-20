// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use crypto::hash::OperationHash;
use tezos_api::ffi::{OperationClassification, PrevalidatorWrapper};
use tezos_messages::p2p::encoding::prelude::Operation;

use crate::{EnablingCondition, State};

use super::{MempoolValidatorState, MempoolValidatorValidateResult};

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MempoolValidatorInitAction {}

impl EnablingCondition<State> for MempoolValidatorInitAction {
    fn is_enabled(&self, state: &State) -> bool {
        state.protocol_runner.is_ready() && state.current_head.get().is_some()
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MempoolValidatorPendingAction {}

impl EnablingCondition<State> for MempoolValidatorPendingAction {
    fn is_enabled(&self, state: &State) -> bool {
        matches!(&state.mempool.validator, MempoolValidatorState::Init { .. })
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MempoolValidatorSuccessAction {
    pub prevalidator: PrevalidatorWrapper,
}

impl EnablingCondition<State> for MempoolValidatorSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.mempool.validator {
            MempoolValidatorState::Pending { block_hash, .. } => {
                &self.prevalidator.predecessor == block_hash
            }
            _ => false,
        }
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MempoolValidatorReadyAction {}

impl EnablingCondition<State> for MempoolValidatorReadyAction {
    fn is_enabled(&self, state: &State) -> bool {
        matches!(
            &state.mempool.validator,
            MempoolValidatorState::Success { .. }
        )
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MempoolValidatorValidateInitAction {
    pub op_hash: OperationHash,
    pub op_content: Operation,
}

impl EnablingCondition<State> for MempoolValidatorValidateInitAction {
    fn is_enabled(&self, state: &State) -> bool {
        state.mempool.validator.is_ready() && !state.mempool.validator.validate_is_pending()
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MempoolValidatorValidatePendingAction {}

impl EnablingCondition<State> for MempoolValidatorValidatePendingAction {
    fn is_enabled(&self, state: &State) -> bool {
        state.mempool.validator.validate_is_init()
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MempoolValidatorValidateSuccessAction {
    pub op_hash: OperationHash,
    pub result: MempoolValidatorValidateResult,
    pub protocol_preapply_start: f64,
    pub protocol_preapply_end: f64,
}

impl EnablingCondition<State> for MempoolValidatorValidateSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .mempool
            .validator
            .validate_pending_op_hash()
            .map_or(false, |hash| hash == &self.op_hash)
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MempoolValidatorReclassifyOperationAction {
    pub op_hash: OperationHash,
    pub classification: OperationClassification,
}

impl EnablingCondition<State> for MempoolValidatorReclassifyOperationAction {
    fn is_enabled(&self, _state: &State) -> bool {
        // TODO: what is the proper enabling condition? check that it is already classified?
        true
    }
}
