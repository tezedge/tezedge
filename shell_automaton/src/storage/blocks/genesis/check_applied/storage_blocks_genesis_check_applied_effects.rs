// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::protocol_runner::init::ProtocolRunnerInitCheckGenesisAppliedSuccessAction;
use crate::service::storage_service::{
    StorageRequestPayload, StorageResponseError, StorageResponseSuccess,
};
use crate::storage::request::StorageRequestCreateAction;
use crate::{Action, ActionWithMeta, Service, Store};

use super::{
    StorageBlocksGenesisCheckAppliedGetMetaErrorAction,
    StorageBlocksGenesisCheckAppliedGetMetaPendingAction,
    StorageBlocksGenesisCheckAppliedGetMetaSuccessAction, StorageBlocksGenesisCheckAppliedState,
    StorageBlocksGenesisCheckAppliedSuccessAction,
};

pub fn storage_blocks_genesis_check_applied_effects<S>(
    store: &mut Store<S>,
    action: &ActionWithMeta,
) where
    S: Service,
{
    let state = store.state.get();

    match &action.action {
        Action::StorageBlocksGenesisCheckAppliedInit(_) => {
            let genesis_hash = state
                .config
                .init_storage_data
                .genesis_block_header_hash
                .clone();
            store.dispatch(StorageRequestCreateAction {
                payload: StorageRequestPayload::BlockMetaGet(genesis_hash.into()),
            });
            store.dispatch(StorageBlocksGenesisCheckAppliedGetMetaPendingAction {});
        }
        Action::StorageRequestError(content) => {
            let req_id = match &state.storage.blocks.genesis.check_applied {
                StorageBlocksGenesisCheckAppliedState::GetMetaPending { req_id } => *req_id,
                _ => return,
            };
            if req_id == content.req_id {
                let _ = match &content.error {
                    StorageResponseError::BlockMetaGetError(_, err) => err.clone(),
                    _ => return,
                };
                store.dispatch(StorageBlocksGenesisCheckAppliedGetMetaErrorAction {});
            }
        }
        Action::StorageBlocksGenesisCheckAppliedGetMetaError(_) => {
            todo!();
        }
        Action::StorageRequestSuccess(content) => {
            let req_id = match &state.storage.blocks.genesis.check_applied {
                StorageBlocksGenesisCheckAppliedState::GetMetaPending { req_id } => *req_id,
                _ => return,
            };
            if req_id == content.req_id {
                let meta = match &content.result {
                    StorageResponseSuccess::BlockMetaGetSuccess(_, meta) => meta.clone(),
                    _ => return,
                };
                store.dispatch(StorageBlocksGenesisCheckAppliedGetMetaSuccessAction { meta });
            }
        }
        Action::StorageBlocksGenesisCheckAppliedGetMetaSuccess(_) => {
            let is_applied = match &state.storage.blocks.genesis.check_applied {
                StorageBlocksGenesisCheckAppliedState::GetMetaSuccess { meta } => {
                    meta.as_ref().map_or(false, |meta| meta.is_applied())
                }
                _ => return,
            };

            store.dispatch(StorageBlocksGenesisCheckAppliedSuccessAction { is_applied });
        }
        Action::StorageBlocksGenesisCheckAppliedSuccess(content) => {
            store.dispatch(ProtocolRunnerInitCheckGenesisAppliedSuccessAction {
                is_applied: content.is_applied,
            });
        }
        _ => {}
    }
}
