// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crypto::hash::{BlockHash, ProtocolHash};
use tezos_messages::p2p::encoding::block_header::BlockHeader;

use crate::{protocol_runner::ProtocolRunnerToken, EnablingCondition, State};

use super::{ContextError, CycleEras, CycleErasQueryState, KvStoreError};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
pub struct RightsCycleErasGetAction {
    pub protocol_hash: ProtocolHash,
    pub block_hash: BlockHash,
    pub block_header: BlockHeader,
}

impl EnablingCondition<State> for RightsCycleErasGetAction {
    fn is_enabled(&self, state: &State) -> bool {
        !state.rights.cycle_eras.contains_key(&self.protocol_hash)
    }
}

macro_rules! enabled_with_state {
    ($action:ident, $state:pat) => {
        impl EnablingCondition<State> for $action {
            fn is_enabled(&self, state: &State) -> bool {
                state
                    .rights
                    .cycle_eras
                    .get(&self.protocol_hash)
                    .map_or(false, |query| matches!(query.state, $state))
            }
        }
    };
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
pub struct RightsCycleErasKVSuccessAction {
    pub protocol_hash: ProtocolHash,
    pub cycle_eras: CycleEras,
}

enabled_with_state!(
    RightsCycleErasKVSuccessAction,
    CycleErasQueryState::PendingKV
);

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
pub struct RightsCycleErasKVErrorAction {
    pub protocol_hash: ProtocolHash,
    pub kv_store_error: KvStoreError,
}

enabled_with_state!(RightsCycleErasKVErrorAction, CycleErasQueryState::PendingKV);

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
pub struct RightsCycleErasContextRequestedAction {
    pub protocol_hash: ProtocolHash,
    pub token: ProtocolRunnerToken,
}

enabled_with_state!(
    RightsCycleErasContextRequestedAction,
    CycleErasQueryState::PendingContext(..)
);

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
pub struct RightsCycleErasContextSuccessAction {
    pub protocol_hash: ProtocolHash,
    pub cycle_eras: CycleEras,
}

enabled_with_state!(
    RightsCycleErasContextSuccessAction,
    CycleErasQueryState::ContextRequested(..)
);

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
pub struct RightsCycleErasContextErrorAction {
    pub protocol_hash: ProtocolHash,
    pub error: ContextError,
}

enabled_with_state!(
    RightsCycleErasContextErrorAction,
    CycleErasQueryState::ContextRequested(..)
);

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
pub struct RightsCycleErasSuccessAction {
    pub protocol_hash: ProtocolHash,
}

enabled_with_state!(
    RightsCycleErasSuccessAction,
    CycleErasQueryState::Success(..)
);

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
pub struct RightsCycleErasErrorAction {
    pub protocol_hash: ProtocolHash,
}

enabled_with_state!(RightsCycleErasErrorAction, CycleErasQueryState::Error(..));
