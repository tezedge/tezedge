// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use tezos_protocol_ipc_messages::GenesisResultDataParams;

use crate::protocol_runner::ProtocolRunnerState;
use crate::service::protocol_runner_service::ProtocolRunnerResult;
use crate::service::ProtocolRunnerService;
use crate::storage::blocks::genesis::init::StorageBlocksGenesisInitState;
use crate::{Action, ActionWithMeta, Service, Store};

use super::{
    StorageBlocksGenesisInitCommitResultGetErrorAction,
    StorageBlocksGenesisInitCommitResultGetPendingAction,
    StorageBlocksGenesisInitCommitResultGetState,
    StorageBlocksGenesisInitCommitResultGetSuccessAction,
};

pub fn storage_blocks_genesis_init_commit_result_get_effects<S>(
    store: &mut Store<S>,
    action: &ActionWithMeta,
) where
    S: Service,
{
    let state = store.state.get();

    match &action.action {
        Action::StorageBlocksGenesisInitCommitResultGetInit(_) => {
            let genesis_commit_hash = match &state.protocol_runner {
                ProtocolRunnerState::Ready(state) => match state.genesis_commit_hash.as_ref() {
                    Some(v) => v.clone(),
                    None => return,
                },
                _ => return,
            };
            let genesis_protocol_hash = state
                .config
                .protocol_runner
                .environment
                .genesis_protocol()
                .unwrap();
            let additional_data = state
                .config
                .protocol_runner
                .environment
                .genesis_additional_data()
                .unwrap();
            let chain_id = state
                .config
                .protocol_runner
                .environment
                .main_chain_id()
                .unwrap();

            let token = store
                .service
                .protocol_runner()
                .genesis_commit_result_get_init(GenesisResultDataParams {
                    genesis_context_hash: genesis_commit_hash,
                    chain_id,
                    genesis_protocol_hash,
                    genesis_max_operations_ttl: additional_data.max_operations_ttl,
                });
            store.dispatch(StorageBlocksGenesisInitCommitResultGetPendingAction { token });
        }
        Action::ProtocolRunnerResponse(content) => {
            let target_token = match &state.storage.blocks.genesis.init {
                StorageBlocksGenesisInitState::CommitResultGet(
                    StorageBlocksGenesisInitCommitResultGetState::Pending { token, .. },
                ) => token,
                _ => return,
            };

            match &content.result {
                ProtocolRunnerResult::GenesisCommitResultGet((token, result)) => {
                    if target_token != token {
                        return;
                    }
                    match result {
                        Ok(result) => {
                            store.dispatch(StorageBlocksGenesisInitCommitResultGetSuccessAction {
                                commit_result: result.clone(),
                            })
                        }
                        Err(err) => {
                            store.dispatch(StorageBlocksGenesisInitCommitResultGetErrorAction {
                                error: err.clone(),
                            })
                        }
                    };
                }
                _ => {}
            }
        }
        _ => {}
    }
}
