// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use storage::block_meta_storage::Meta;
use storage::{BlockAdditionalData, BlockHeaderWithHash};
use tezos_api::ffi::{ApplyBlockRequest, ApplyBlockResponse};
use tezos_protocol_ipc_client::ProtocolServiceError;

use crypto::hash::BlockHash;

use crate::request::RequestId;
use crate::service::rpc_service::RpcId;
use crate::{EnablingCondition, State};

use super::{BlockApplierApplyError, BlockApplierApplyState};

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlockApplierEnqueueBlockAction {
    pub block_hash: Arc<BlockHash>,
    pub injector_rpc_id: Option<RpcId>,
}

impl EnablingCondition<State> for BlockApplierEnqueueBlockAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlockApplierApplyInitAction {
    pub block_hash: Arc<BlockHash>,
    pub injector_rpc_id: Option<RpcId>,
}

impl EnablingCondition<State> for BlockApplierApplyInitAction {
    fn is_enabled(&self, state: &State) -> bool {
        matches!(
            &state.block_applier.current,
            BlockApplierApplyState::Idle { .. }
                | BlockApplierApplyState::Error { .. }
                | BlockApplierApplyState::Success { .. }
        )
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlockApplierApplyPrepareDataPendingAction {
    pub storage_req_id: RequestId,
}

impl EnablingCondition<State> for BlockApplierApplyPrepareDataPendingAction {
    fn is_enabled(&self, state: &State) -> bool {
        matches!(
            &state.block_applier.current,
            BlockApplierApplyState::Init { .. }
        )
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlockApplierApplyPrepareDataSuccessAction {
    pub block: Arc<BlockHeaderWithHash>,
    pub block_meta: Arc<Meta>,
    pub apply_block_req: Arc<ApplyBlockRequest>,
}

impl EnablingCondition<State> for BlockApplierApplyPrepareDataSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        matches!(
            &state.block_applier.current,
            BlockApplierApplyState::PrepareDataPending { .. }
        )
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlockApplierApplyProtocolRunnerApplyInitAction {}

impl EnablingCondition<State> for BlockApplierApplyProtocolRunnerApplyInitAction {
    fn is_enabled(&self, state: &State) -> bool {
        matches!(
            &state.block_applier.current,
            BlockApplierApplyState::PrepareDataSuccess { .. }
        )
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlockApplierApplyProtocolRunnerApplyPendingAction {}

impl EnablingCondition<State> for BlockApplierApplyProtocolRunnerApplyPendingAction {
    fn is_enabled(&self, state: &State) -> bool {
        matches!(
            &state.block_applier.current,
            BlockApplierApplyState::PrepareDataSuccess { .. }
        )
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlockApplierApplyProtocolRunnerApplyRetryAction {
    /// Block application error because of which we are retrying.
    /// Because of the issues in the cache, we have to retry if block
    /// application fails as failure cleans the cache and retry will
    /// resolve cache related issues.
    pub reason: ProtocolServiceError,
    pub block_hash: Option<Arc<BlockHash>>,
}

impl EnablingCondition<State> for BlockApplierApplyProtocolRunnerApplyRetryAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.block_applier.current {
            BlockApplierApplyState::ProtocolRunnerApplyPending { retry, .. } => retry.is_none(),
            _ => false,
        }
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlockApplierApplyProtocolRunnerApplySuccessAction {
    pub apply_result: Arc<ApplyBlockResponse>,
}

impl EnablingCondition<State> for BlockApplierApplyProtocolRunnerApplySuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        matches!(
            &state.block_applier.current,
            BlockApplierApplyState::ProtocolRunnerApplyPending { .. }
        )
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlockApplierApplyStoreApplyResultPendingAction {
    pub storage_req_id: RequestId,
}

impl EnablingCondition<State> for BlockApplierApplyStoreApplyResultPendingAction {
    fn is_enabled(&self, state: &State) -> bool {
        matches!(
            &state.block_applier.current,
            BlockApplierApplyState::ProtocolRunnerApplySuccess { .. }
        )
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlockApplierApplyStoreApplyResultSuccessAction {
    pub block_additional_data: Arc<BlockAdditionalData>,
}

impl EnablingCondition<State> for BlockApplierApplyStoreApplyResultSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        matches!(
            &state.block_applier.current,
            BlockApplierApplyState::StoreApplyResultPending { .. }
        )
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlockApplierApplyErrorAction {
    pub error: BlockApplierApplyError,
}

impl EnablingCondition<State> for BlockApplierApplyErrorAction {
    fn is_enabled(&self, state: &State) -> bool {
        matches!(
            &state.block_applier.current,
            BlockApplierApplyState::PrepareDataPending { .. }
                | BlockApplierApplyState::PrepareDataSuccess { .. }
                | BlockApplierApplyState::ProtocolRunnerApplyPending { .. }
                | BlockApplierApplyState::StoreApplyResultPending { .. }
        )
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlockApplierApplySuccessAction {}

impl EnablingCondition<State> for BlockApplierApplySuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        matches!(
            &state.block_applier.current,
            BlockApplierApplyState::StoreApplyResultSuccess { .. }
        )
    }
}
