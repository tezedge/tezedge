// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crypto::hash::BlockHash;
use redux_rs::EnablingCondition;
use tezos_messages::{
    base::signature_public_key::SignaturePublicKey,
    p2p::encoding::block_header::{BlockHeader, Level},
};

use crate::State;

use super::{CurrentHeadPrecheckError, CurrentHeadState};

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CurrentHeadReceivedAction {
    pub block_hash: BlockHash,
    pub block_header: BlockHeader,
    pub injected: bool,
}

/// Enables [CurrentHeadReceivedAction] when its level is the next to the one of last applied block
/// and its block hash hasn't seen yet.
impl EnablingCondition<State> for CurrentHeadReceivedAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .current_heads
            .candidate_level()
            .map_or(true, |l| l == self.block_header.level())
            && !state
                .current_heads
                .applied_hashes
                .contains_key(&self.block_hash)
            && !state
                .current_heads
                .candidates
                .contains_key(&self.block_hash)
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CurrentHeadPrecheckAction {
    pub block_hash: BlockHash,
    pub injected: bool,
}

impl EnablingCondition<State> for CurrentHeadPrecheckAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .current_heads
            .applied_head()
            .map_or(false, |applied_head| {
                if let Some(CurrentHeadState::Received { block_header }) =
                    state.current_heads.candidates.get(&self.block_hash)
                {
                    block_header.level() == applied_head.level + 1
                } else {
                    false
                }
            })
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CurrentHeadPrecheckSuccessAction {
    pub block_hash: BlockHash,
    pub baker: SignaturePublicKey,
    pub priority: u16,
    pub injected: bool,
}

impl EnablingCondition<State> for CurrentHeadPrecheckSuccessAction {
    fn is_enabled(&self, _state: &State) -> bool {
        true
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CurrentHeadPrecheckRejectedAction {
    pub block_hash: BlockHash,
}

impl EnablingCondition<State> for CurrentHeadPrecheckRejectedAction {
    fn is_enabled(&self, _state: &State) -> bool {
        true
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CurrentHeadErrorAction {
    pub block_hash: BlockHash,
    pub error: CurrentHeadPrecheckError,
}

impl EnablingCondition<State> for CurrentHeadErrorAction {
    fn is_enabled(&self, _state: &State) -> bool {
        true
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CurrentHeadApplyAction {
    pub block_hash: BlockHash,
    pub level: Level,
    pub timestamp: i64,
}

impl EnablingCondition<State> for CurrentHeadApplyAction {
    fn is_enabled(&self, _state: &State) -> bool {
        true
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CurrentHeadPrecacheBakingRightsAction {}

impl EnablingCondition<State> for CurrentHeadPrecacheBakingRightsAction {
    fn is_enabled(&self, state: &State) -> bool {
        state.current_heads.applied_head().is_some()
    }
}
