// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use tezos_api::ffi::{BeginConstructionRequest, PrevalidatorWrapper, ValidateOperationResponse};
use tezos_messages::p2p::encoding::operation::Operation;

use super::protocol_state::ValidationState;
use crate::{EnablingCondition, State};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProtocolConstructStatefulPrevalidatorStartAction {
    pub request: BeginConstructionRequest,
}

impl EnablingCondition<State> for ProtocolConstructStatefulPrevalidatorStartAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.protocol.validation_state {
            // construction is already in progress
            ValidationState::NotReady { .. } => false,
            _ => true,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProtocolConstructStatefulPrevalidatorDoneAction {
    pub prevalidator: PrevalidatorWrapper,
}

impl EnablingCondition<State> for ProtocolConstructStatefulPrevalidatorDoneAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.protocol.validation_state {
            ValidationState::NotReady { .. } | ValidationState::Initial => true,
            _ => false,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProtocolConstructStatelessPrevalidatorStartAction {
    pub request: BeginConstructionRequest,
}

impl EnablingCondition<State> for ProtocolConstructStatelessPrevalidatorStartAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.protocol.validation_state {
            // construction is already in progress
            ValidationState::NotReady { .. } => false,
            _ => true,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProtocolConstructStatelessPrevalidatorDoneAction {
    pub prevalidator: PrevalidatorWrapper,
}

impl EnablingCondition<State> for ProtocolConstructStatelessPrevalidatorDoneAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.protocol.validation_state {
            ValidationState::NotReady { .. } | ValidationState::Initial => true,
            _ => false,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProtocolValidateOperationStartAction {
    pub operation: Operation,
}

impl EnablingCondition<State> for ProtocolValidateOperationStartAction {
    fn is_enabled(&self, state: &State) -> bool {
        let _ = state;
        true
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProtocolValidateOperationDoneAction {
    pub response: ValidateOperationResponse,
}

impl EnablingCondition<State> for ProtocolValidateOperationDoneAction {
    fn is_enabled(&self, state: &State) -> bool {
        matches!(
            &state.protocol.validation_state,
            ValidationState::Validating { .. }
        )
    }
}
