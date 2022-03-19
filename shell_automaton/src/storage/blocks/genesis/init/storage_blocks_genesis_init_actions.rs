// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use crypto::hash::ContextHash;

use crate::{EnablingCondition, State};

use super::commit_result_put::StorageBlocksGenesisInitCommitResultPutState;
use super::StorageBlocksGenesisInitState;

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageBlocksGenesisInitAction {
    pub genesis_commit_hash: ContextHash,
}

impl EnablingCondition<State> for StorageBlocksGenesisInitAction {
    fn is_enabled(&self, state: &State) -> bool {
        matches!(
            &state.storage.blocks.genesis.init,
            StorageBlocksGenesisInitState::Idle | StorageBlocksGenesisInitState::Success
        )
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageBlocksGenesisInitSuccessAction {}

impl EnablingCondition<State> for StorageBlocksGenesisInitSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        matches!(
            &state.storage.blocks.genesis.init,
            StorageBlocksGenesisInitState::CommitResultPut(
                StorageBlocksGenesisInitCommitResultPutState::Success { .. },
            )
        )
    }
}
