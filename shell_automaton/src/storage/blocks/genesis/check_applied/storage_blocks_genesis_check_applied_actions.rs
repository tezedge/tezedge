// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use storage::block_meta_storage::Meta;

use crate::{EnablingCondition, State};

use super::StorageBlocksGenesisCheckAppliedState;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageBlocksGenesisCheckAppliedInitAction {}

impl EnablingCondition<State> for StorageBlocksGenesisCheckAppliedInitAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.storage.blocks.genesis.check_applied {
            StorageBlocksGenesisCheckAppliedState::Idle => true,
            StorageBlocksGenesisCheckAppliedState::GetMetaError { .. } => true,
            StorageBlocksGenesisCheckAppliedState::Success { .. } => true,
            _ => false,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageBlocksGenesisCheckAppliedGetMetaPendingAction {}

impl EnablingCondition<State> for StorageBlocksGenesisCheckAppliedGetMetaPendingAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.storage.blocks.genesis.check_applied {
            StorageBlocksGenesisCheckAppliedState::GetMetaInit => true,
            _ => false,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageBlocksGenesisCheckAppliedGetMetaErrorAction {}

impl EnablingCondition<State> for StorageBlocksGenesisCheckAppliedGetMetaErrorAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.storage.blocks.genesis.check_applied {
            StorageBlocksGenesisCheckAppliedState::GetMetaPending { .. } => true,
            _ => false,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageBlocksGenesisCheckAppliedGetMetaSuccessAction {
    pub meta: Option<Meta>,
}

impl EnablingCondition<State> for StorageBlocksGenesisCheckAppliedGetMetaSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        self.meta.as_ref().map_or(true, |meta| meta.level() == 0)
            && match &state.storage.blocks.genesis.check_applied {
                StorageBlocksGenesisCheckAppliedState::GetMetaPending { .. } => true,
                _ => false,
            }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageBlocksGenesisCheckAppliedSuccessAction {
    pub is_applied: bool,
}

impl EnablingCondition<State> for StorageBlocksGenesisCheckAppliedSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.storage.blocks.genesis.check_applied {
            StorageBlocksGenesisCheckAppliedState::GetMetaSuccess { meta } => {
                meta.as_ref().map_or(false, |meta| meta.is_applied()) == self.is_applied
            }
            _ => false,
        }
    }
}
