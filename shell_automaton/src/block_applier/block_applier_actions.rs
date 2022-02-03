// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use storage::block_meta_storage::Meta;
use storage::{BlockAdditionalData, BlockHeaderWithHash};
use tezos_api::ffi::{ApplyBlockError, ApplyBlockRequest, ApplyBlockResponse};

use crypto::hash::{BlockHash, ChainId};

use crate::request::RequestId;
use crate::{EnablingCondition, State};

use super::{BlockApplierApplyError, BlockApplierApplyState};

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlockApplierEnqueueBlockAction {
    pub chain_id: Arc<ChainId>,
    pub block_hash: Arc<BlockHash>,
}

impl EnablingCondition<State> for BlockApplierEnqueueBlockAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlockApplierApplyInitAction {
    pub chain_id: Arc<ChainId>,
    pub block_hash: Arc<BlockHash>,
}

impl EnablingCondition<State> for BlockApplierApplyInitAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.block_applier.current {
            BlockApplierApplyState::Idle { .. } => true,
            BlockApplierApplyState::Error { .. } => true,
            BlockApplierApplyState::Success { .. } => true,
            _ => false,
        }
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlockApplierApplyPrepareDataPendingAction {
    pub storage_req_id: RequestId,
}

impl EnablingCondition<State> for BlockApplierApplyPrepareDataPendingAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.block_applier.current {
            BlockApplierApplyState::Init { .. } => true,
            _ => false,
        }
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlockApplierApplyPrepareDataSuccessAction {
    pub block: Arc<BlockHeaderWithHash>,
    pub block_meta: Arc<Meta>,
    pub apply_block_req: Arc<ApplyBlockRequest>,
}

impl EnablingCondition<State> for BlockApplierApplyPrepareDataSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.block_applier.current {
            BlockApplierApplyState::PrepareDataPending { .. } => true,
            _ => false,
        }
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlockApplierApplyProtocolRunnerApplyInitAction {}

impl EnablingCondition<State> for BlockApplierApplyProtocolRunnerApplyInitAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.block_applier.current {
            BlockApplierApplyState::PrepareDataSuccess { .. } => true,
            _ => false,
        }
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlockApplierApplyProtocolRunnerApplyPendingAction {}

impl EnablingCondition<State> for BlockApplierApplyProtocolRunnerApplyPendingAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.block_applier.current {
            BlockApplierApplyState::PrepareDataSuccess { .. } => true,
            _ => false,
        }
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlockApplierApplyProtocolRunnerApplyRetryAction {
    /// Block application error because of which we are retrying.
    /// Because of the issues in the cache, we have to retry if block
    /// application fails as failure cleans the cache and retry will
    /// resolve cache related issues.
    pub reason: ApplyBlockError,
}

impl EnablingCondition<State> for BlockApplierApplyProtocolRunnerApplyRetryAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.block_applier.current {
            BlockApplierApplyState::ProtocolRunnerApplyPending { retry, .. } => retry.is_none(),
            _ => false,
        }
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlockApplierApplyProtocolRunnerApplySuccessAction {
    pub apply_result: Arc<ApplyBlockResponse>,
}

impl EnablingCondition<State> for BlockApplierApplyProtocolRunnerApplySuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.block_applier.current {
            BlockApplierApplyState::ProtocolRunnerApplyPending { .. } => true,
            _ => false,
        }
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlockApplierApplyStoreApplyResultPendingAction {
    pub storage_req_id: RequestId,
}

impl EnablingCondition<State> for BlockApplierApplyStoreApplyResultPendingAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.block_applier.current {
            BlockApplierApplyState::ProtocolRunnerApplySuccess { .. } => true,
            _ => false,
        }
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlockApplierApplyStoreApplyResultSuccessAction {
    pub block_additional_data: Arc<BlockAdditionalData>,
}

impl EnablingCondition<State> for BlockApplierApplyStoreApplyResultSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.block_applier.current {
            BlockApplierApplyState::StoreApplyResultPending { .. } => true,
            _ => false,
        }
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlockApplierApplyErrorAction {
    pub error: BlockApplierApplyError,
}

impl EnablingCondition<State> for BlockApplierApplyErrorAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.block_applier.current {
            BlockApplierApplyState::PrepareDataPending { .. }
            | BlockApplierApplyState::ProtocolRunnerApplyPending { .. }
            | BlockApplierApplyState::StoreApplyResultPending { .. } => true,
            _ => false,
        }
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlockApplierApplySuccessAction {}

impl EnablingCondition<State> for BlockApplierApplySuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.block_applier.current {
            BlockApplierApplyState::StoreApplyResultSuccess { .. } => true,
            _ => false,
        }
    }
}
