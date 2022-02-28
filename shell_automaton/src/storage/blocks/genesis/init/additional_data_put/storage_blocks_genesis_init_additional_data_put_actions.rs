// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use crate::storage::blocks::genesis::init::header_put::StorageBlocksGenesisInitHeaderPutState;
use crate::storage::blocks::genesis::init::StorageBlocksGenesisInitState;
use crate::{EnablingCondition, State};

use super::StorageBlocksGenesisInitAdditionalDataPutState;

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageBlocksGenesisInitAdditionalDataPutInitAction {}

impl EnablingCondition<State> for StorageBlocksGenesisInitAdditionalDataPutInitAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.storage.blocks.genesis.init {
            StorageBlocksGenesisInitState::HeaderPut(
                StorageBlocksGenesisInitHeaderPutState::Success { .. },
            ) => true,
            _ => false,
        }
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageBlocksGenesisInitAdditionalDataPutPendingAction {}

impl EnablingCondition<State> for StorageBlocksGenesisInitAdditionalDataPutPendingAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.storage.blocks.genesis.init {
            StorageBlocksGenesisInitState::AdditionalDataPut(
                StorageBlocksGenesisInitAdditionalDataPutState::Init {},
            ) => true,
            _ => false,
        }
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageBlocksGenesisInitAdditionalDataPutErrorAction {}

impl EnablingCondition<State> for StorageBlocksGenesisInitAdditionalDataPutErrorAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.storage.blocks.genesis.init {
            StorageBlocksGenesisInitState::AdditionalDataPut(
                StorageBlocksGenesisInitAdditionalDataPutState::Pending { .. },
            ) => true,
            _ => false,
        }
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageBlocksGenesisInitAdditionalDataPutSuccessAction {}

impl EnablingCondition<State> for StorageBlocksGenesisInitAdditionalDataPutSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.storage.blocks.genesis.init {
            StorageBlocksGenesisInitState::AdditionalDataPut(
                StorageBlocksGenesisInitAdditionalDataPutState::Pending { .. },
            ) => true,
            _ => false,
        }
    }
}
