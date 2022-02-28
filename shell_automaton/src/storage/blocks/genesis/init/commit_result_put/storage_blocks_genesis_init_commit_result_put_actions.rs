// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use crate::storage::blocks::genesis::init::commit_result_get::StorageBlocksGenesisInitCommitResultGetState;
use crate::storage::blocks::genesis::init::StorageBlocksGenesisInitState;
use crate::{EnablingCondition, State};

use super::StorageBlocksGenesisInitCommitResultPutState;

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageBlocksGenesisInitCommitResultPutInitAction {}

impl EnablingCondition<State> for StorageBlocksGenesisInitCommitResultPutInitAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.storage.blocks.genesis.init {
            StorageBlocksGenesisInitState::CommitResultGet(
                StorageBlocksGenesisInitCommitResultGetState::Success { .. },
            ) => true,
            _ => false,
        }
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageBlocksGenesisInitCommitResultPutErrorAction {}

impl EnablingCondition<State> for StorageBlocksGenesisInitCommitResultPutErrorAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.storage.blocks.genesis.init {
            StorageBlocksGenesisInitState::CommitResultPut(
                StorageBlocksGenesisInitCommitResultPutState::Init { .. },
            ) => true,
            _ => false,
        }
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageBlocksGenesisInitCommitResultPutSuccessAction {}

impl EnablingCondition<State> for StorageBlocksGenesisInitCommitResultPutSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.storage.blocks.genesis.init {
            StorageBlocksGenesisInitState::CommitResultPut(
                StorageBlocksGenesisInitCommitResultPutState::Init { .. },
            ) => true,
            _ => false,
        }
    }
}
