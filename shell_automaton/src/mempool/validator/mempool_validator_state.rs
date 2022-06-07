// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use crypto::hash::{BlockHash, OperationHash};
use tezos_api::ffi::{Applied, Errored, PrevalidatorWrapper};
use tezos_messages::p2p::encoding::prelude::Operation;

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum MempoolValidatorValidateResult {
    Applied(Applied),
    Refused(Errored),
    BranchRefused(Errored),
    BranchDelayed(Errored),
    Outdated(Errored),
    Unparseable(OperationHash),
}

impl MempoolValidatorValidateResult {
    pub fn is_applied(&self) -> bool {
        matches!(self, Self::Applied(_))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum MempoolValidatorValidateState {
    Idle {
        time: u64,
    },
    Init {
        time: u64,
        op_hash: OperationHash,
        op_content: Operation,
    },
    Pending {
        time: u64,
        op_hash: OperationHash,
        op_content: Operation,
    },
    Success {
        time: u64,
        op_hash: OperationHash,
        result: MempoolValidatorValidateResult,
        protocol_preapply_start: f64,
        protocol_preapply_end: f64,
    },
}

impl MempoolValidatorValidateState {
    pub fn is_init(&self) -> bool {
        matches!(self, Self::Init { .. })
    }

    pub fn is_pending(&self) -> bool {
        matches!(self, Self::Pending { .. })
    }

    pub fn pending_op_hash(&self) -> Option<&OperationHash> {
        match self {
            Self::Pending { op_hash, .. } => Some(op_hash),
            _ => None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum MempoolValidatorState {
    Idle {
        time: u64,
    },
    Init {
        time: u64,
    },
    Pending {
        time: u64,
        block_hash: BlockHash,
    },
    Success {
        time: u64,
        prevalidator: PrevalidatorWrapper,
    },
    Ready {
        time: u64,
        prevalidator: PrevalidatorWrapper,
        validate: MempoolValidatorValidateState,
    },
}

impl MempoolValidatorState {
    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready { .. })
    }

    pub fn prevalidator(&self) -> Option<&PrevalidatorWrapper> {
        match self {
            Self::Success { prevalidator, .. } => Some(prevalidator),
            Self::Ready { prevalidator, .. } => Some(prevalidator),
            _ => None,
        }
    }

    pub fn prevalidator_matches(&self, other: &PrevalidatorWrapper) -> bool {
        self.prevalidator()
            .map_or(false, |p| p.predecessor == other.predecessor)
    }

    pub fn validate(&self) -> Option<&MempoolValidatorValidateState> {
        match self {
            Self::Ready { validate, .. } => Some(validate),
            _ => None,
        }
    }

    pub fn validate_is_init(&self) -> bool {
        self.validate().map_or(false, |v| v.is_init())
    }

    pub fn validate_is_pending(&self) -> bool {
        self.validate().map_or(false, |v| v.is_pending())
    }

    pub fn validate_pending_op_hash(&self) -> Option<&OperationHash> {
        self.validate().and_then(|v| v.pending_op_hash())
    }
}

impl Default for MempoolValidatorState {
    fn default() -> Self {
        Self::Idle { time: 0 }
    }
}
