// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crypto::hash::ContextHash;
use serde::{Deserialize, Serialize};

use crate::storage::blocks::genesis::init::StorageBlocksGenesisInitState;
use crate::{EnablingCondition, State};

use super::StorageBlocksGenesisInitHeaderPutState;

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageBlocksGenesisInitHeaderPutInitAction {
    pub genesis_commit_hash: ContextHash,
}

impl EnablingCondition<State> for StorageBlocksGenesisInitHeaderPutInitAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.storage.blocks.genesis.init {
            StorageBlocksGenesisInitState::Idle => true,
            StorageBlocksGenesisInitState::Success => true,
            _ => false,
        }
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageBlocksGenesisInitHeaderPutPendingAction {}

impl EnablingCondition<State> for StorageBlocksGenesisInitHeaderPutPendingAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.storage.blocks.genesis.init {
            StorageBlocksGenesisInitState::HeaderPut(
                StorageBlocksGenesisInitHeaderPutState::Init,
            ) => true,
            _ => false,
        }
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageBlocksGenesisInitHeaderPutErrorAction {}

impl EnablingCondition<State> for StorageBlocksGenesisInitHeaderPutErrorAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.storage.blocks.genesis.init {
            StorageBlocksGenesisInitState::HeaderPut(
                StorageBlocksGenesisInitHeaderPutState::Pending { .. },
            ) => true,
            _ => false,
        }
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageBlocksGenesisInitHeaderPutSuccessAction {
    pub is_new_block: bool,
}

impl EnablingCondition<State> for StorageBlocksGenesisInitHeaderPutSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.storage.blocks.genesis.init {
            StorageBlocksGenesisInitState::HeaderPut(
                StorageBlocksGenesisInitHeaderPutState::Pending { .. },
            ) => true,
            _ => false,
        }
    }
}
