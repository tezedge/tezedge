// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crypto::hash::ProtocolHash;
use tezos_api::ffi::PrevalidatorWrapper;
use tezos_messages::p2p::encoding::operation::Operation;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProtocolState {
    pub validation_state: ValidationState,
    // queue to validate
    pub operations_to_validate: VecDeque<Operation>,
    // endorsements should go first
    pub endorsements_to_validate: VecDeque<Operation>,
}

impl Default for ProtocolState {
    fn default() -> Self {
        ProtocolState {
            validation_state: ValidationState::Initial,
            // how many operations could be in the block?
            operations_to_validate: VecDeque::with_capacity(256),
            endorsements_to_validate: VecDeque::with_capacity(256),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ValidationState {
    // mempool just started, no prevalidator created yet
    Initial,
    // creating a new prevalidator, should not validate operations in this state
    // should wait until the prevalidator created
    NotReady {
        protocol: ProtocolHash,
    },
    // has a prevalidator
    Ready {
        prevalidator: PrevalidatorWrapper,
        // whether prevalidator is stateful or stateless
        stateful: bool,
    },
    // validation is in progress
    Validating {
        prevalidator: PrevalidatorWrapper,
        operation: Operation,
        stateful: bool,
    },
}

impl ValidationState {
    pub fn protocol(&self) -> Option<ProtocolHash> {
        match self {
            ValidationState::Initial => None,
            ValidationState::NotReady { protocol } => Some(protocol.clone()),
            ValidationState::Ready { prevalidator, .. }
            | ValidationState::Validating { prevalidator, .. } => {
                Some(prevalidator.protocol.clone())
            }
        }
    }
}
