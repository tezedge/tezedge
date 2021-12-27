// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crypto::hash::ProtocolHash;
use tezos_api::ffi::PrevalidatorWrapper;
use tezos_messages::p2p::encoding::operation::Operation;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProtocolState {
    pub operation_validation_state: OperationValidationState,
    // queue to validate
    pub operations_to_validate: VecDeque<Operation>,
    // endorsements should go first
    pub endorsements_to_validate: VecDeque<Operation>,
}

impl Default for ProtocolState {
    fn default() -> Self {
        ProtocolState {
            operation_validation_state: OperationValidationState::Initial,
            // how many operations could be in the block?
            operations_to_validate: VecDeque::with_capacity(256),
            endorsements_to_validate: VecDeque::with_capacity(256),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum OperationValidationState {
    // mempool just started, no prevalidator created yet
    Initial,
    // creating a new prevalidator, should not validate operations in this state
    // if an operation arrived when in this state, the operation should be enqueued
    WaitingPrevalidator {
        protocol: ProtocolHash,
    },
    // the prevalidator is ready, but the is no operation to validate
    WaitingOperations {
        prevalidator: PrevalidatorWrapper,
        // whether the prevalidator is stateful or stateless
        stateful: bool,
    },
    // there is an operation to validate
    ValidatingOperation {
        prevalidator: PrevalidatorWrapper,
        operation: Operation,
        stateful: bool,
    },
}

impl OperationValidationState {
    pub fn protocol(&self) -> Option<ProtocolHash> {
        match self {
            OperationValidationState::Initial => None,
            OperationValidationState::WaitingPrevalidator { protocol } => Some(protocol.clone()),
            OperationValidationState::WaitingOperations { prevalidator, .. }
            | OperationValidationState::ValidatingOperation { prevalidator, .. } => {
                Some(prevalidator.protocol.clone())
            }
        }
    }
}
