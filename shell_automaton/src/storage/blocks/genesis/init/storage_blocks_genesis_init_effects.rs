// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::protocol_runner::ProtocolRunnerNotifyStatusAction;
use crate::{Action, ActionWithMeta, Service, Store};

use super::additional_data_put::StorageBlocksGenesisInitAdditionalDataPutInitAction;
use super::commit_result_get::StorageBlocksGenesisInitCommitResultGetInitAction;
use super::commit_result_put::StorageBlocksGenesisInitCommitResultPutInitAction;
use super::header_put::StorageBlocksGenesisInitHeaderPutInitAction;
use super::StorageBlocksGenesisInitSuccessAction;

pub fn storage_blocks_genesis_init_effects<S>(store: &mut Store<S>, action: &ActionWithMeta)
where
    S: Service,
{
    match &action.action {
        Action::StorageBlocksGenesisInit(content) => {
            store.dispatch(StorageBlocksGenesisInitHeaderPutInitAction {
                genesis_commit_hash: content.genesis_commit_hash.clone(),
            });
        }
        Action::StorageBlocksGenesisInitHeaderPutSuccess(_) => {
            store.dispatch(StorageBlocksGenesisInitAdditionalDataPutInitAction {});
        }
        Action::StorageBlocksGenesisInitAdditionalDataPutSuccess(_) => {
            store.dispatch(StorageBlocksGenesisInitCommitResultGetInitAction {});
        }
        Action::StorageBlocksGenesisInitCommitResultGetSuccess(_) => {
            store.dispatch(StorageBlocksGenesisInitCommitResultPutInitAction {});
        }
        Action::StorageBlocksGenesisInitCommitResultPutSuccess(_) => {
            store.dispatch(StorageBlocksGenesisInitSuccessAction {});
        }
        Action::StorageBlocksGenesisInitSuccess(_) => {
            store.dispatch(ProtocolRunnerNotifyStatusAction {});
        }
        _ => {}
    }
}
