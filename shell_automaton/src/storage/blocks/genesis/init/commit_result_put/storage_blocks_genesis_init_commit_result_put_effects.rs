// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::service::StorageService;
use crate::storage::blocks::genesis::init::StorageBlocksGenesisInitState;
use crate::{Action, ActionWithMeta, Service, Store};

use super::{
    StorageBlocksGenesisInitCommitResultPutErrorAction,
    StorageBlocksGenesisInitCommitResultPutState,
    StorageBlocksGenesisInitCommitResultPutSuccessAction,
};

pub fn storage_blocks_genesis_init_commit_result_put_effects<S>(
    store: &mut Store<S>,
    action: &ActionWithMeta,
) where
    S: Service,
{
    let state = store.state.get();

    if let Action::StorageBlocksGenesisInitCommitResultPutInit(_) = &action.action {
        let init_storage_info = &state.config.init_storage_data;
        let result = match &state.storage.blocks.genesis.init {
            StorageBlocksGenesisInitState::CommitResultPut(
                StorageBlocksGenesisInitCommitResultPutState::Init { result },
            ) => result.clone(),
            _ => return,
        };

        match store
            .service
            .storage()
            .blocks_genesis_commit_result_put(init_storage_info, result)
        {
            Ok(_) => store.dispatch(StorageBlocksGenesisInitCommitResultPutSuccessAction {}),
            Err(_) => store.dispatch(StorageBlocksGenesisInitCommitResultPutErrorAction {}),
        };
    }
}
