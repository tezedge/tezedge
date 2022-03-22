// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crypto::hash::ContextHash;
use serde::{Deserialize, Serialize};

use crate::storage::blocks::genesis::init::StorageBlocksGenesisInitState;
use crate::{EnablingCondition, State};

use super::StorageBlocksGenesisInitHeaderPutState;

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageBlocksGenesisInitHeaderPutInitAction {
    pub genesis_commit_hash: ContextHash,
}

impl EnablingCondition<State> for StorageBlocksGenesisInitHeaderPutInitAction {
    fn is_enabled(&self, state: &State) -> bool {
        matches!(
            &state.storage.blocks.genesis.init,
            StorageBlocksGenesisInitState::Idle | StorageBlocksGenesisInitState::Success
        )
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageBlocksGenesisInitHeaderPutPendingAction {}

impl EnablingCondition<State> for StorageBlocksGenesisInitHeaderPutPendingAction {
    fn is_enabled(&self, state: &State) -> bool {
        matches!(
            &state.storage.blocks.genesis.init,
            StorageBlocksGenesisInitState::HeaderPut(StorageBlocksGenesisInitHeaderPutState::Init,)
        )
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageBlocksGenesisInitHeaderPutErrorAction {}

impl EnablingCondition<State> for StorageBlocksGenesisInitHeaderPutErrorAction {
    fn is_enabled(&self, state: &State) -> bool {
        matches!(
            &state.storage.blocks.genesis.init,
            StorageBlocksGenesisInitState::HeaderPut(
                StorageBlocksGenesisInitHeaderPutState::Pending { .. },
            )
        )
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageBlocksGenesisInitHeaderPutSuccessAction {
    pub is_new_block: bool,
}

impl EnablingCondition<State> for StorageBlocksGenesisInitHeaderPutSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        matches!(
            &state.storage.blocks.genesis.init,
            StorageBlocksGenesisInitState::HeaderPut(
                StorageBlocksGenesisInitHeaderPutState::Pending { .. },
            )
        )
    }
}
