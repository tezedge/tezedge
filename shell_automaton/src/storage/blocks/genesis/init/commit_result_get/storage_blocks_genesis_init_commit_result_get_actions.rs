// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use tezos_api::ffi::CommitGenesisResult;
use tezos_protocol_ipc_client::ProtocolServiceError;

use crate::protocol_runner::ProtocolRunnerToken;
use crate::storage::blocks::genesis::init::additional_data_put::StorageBlocksGenesisInitAdditionalDataPutState;
use crate::storage::blocks::genesis::init::StorageBlocksGenesisInitState;
use crate::{EnablingCondition, State};

use super::StorageBlocksGenesisInitCommitResultGetState;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageBlocksGenesisInitCommitResultGetInitAction {}

impl EnablingCondition<State> for StorageBlocksGenesisInitCommitResultGetInitAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.storage.blocks.genesis.init {
            StorageBlocksGenesisInitState::AdditionalDataPut(
                StorageBlocksGenesisInitAdditionalDataPutState::Success { .. },
            ) => true,
            _ => false,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageBlocksGenesisInitCommitResultGetPendingAction {
    pub token: ProtocolRunnerToken,
}

impl EnablingCondition<State> for StorageBlocksGenesisInitCommitResultGetPendingAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.storage.blocks.genesis.init {
            StorageBlocksGenesisInitState::CommitResultGet(
                StorageBlocksGenesisInitCommitResultGetState::Init { .. },
            ) => true,
            _ => false,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageBlocksGenesisInitCommitResultGetErrorAction {
    pub error: ProtocolServiceError,
}

impl EnablingCondition<State> for StorageBlocksGenesisInitCommitResultGetErrorAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.storage.blocks.genesis.init {
            StorageBlocksGenesisInitState::CommitResultGet(
                StorageBlocksGenesisInitCommitResultGetState::Pending { .. },
            ) => true,
            _ => false,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageBlocksGenesisInitCommitResultGetSuccessAction {
    pub commit_result: CommitGenesisResult,
}

impl EnablingCondition<State> for StorageBlocksGenesisInitCommitResultGetSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.storage.blocks.genesis.init {
            StorageBlocksGenesisInitState::CommitResultGet(
                StorageBlocksGenesisInitCommitResultGetState::Pending { .. },
            ) => true,
            _ => false,
        }
    }
}
