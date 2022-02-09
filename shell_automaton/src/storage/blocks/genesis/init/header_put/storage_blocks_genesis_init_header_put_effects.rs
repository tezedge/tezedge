// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use storage::BlockHeaderWithHash;
use tezos_api::environment::get_empty_operation_list_list_hash;

use crate::service::storage_service::{
    StorageRequestPayload, StorageResponseError, StorageResponseSuccess,
};
use crate::storage::blocks::genesis::init::StorageBlocksGenesisInitState;
use crate::storage::request::StorageRequestCreateAction;
use crate::{Action, ActionWithMeta, Service, Store};

use super::{
    StorageBlocksGenesisInitHeaderPutErrorAction, StorageBlocksGenesisInitHeaderPutPendingAction,
    StorageBlocksGenesisInitHeaderPutState, StorageBlocksGenesisInitHeaderPutSuccessAction,
};

pub fn storage_blocks_genesis_init_header_put_effects<S>(
    store: &mut Store<S>,
    action: &ActionWithMeta,
) where
    S: Service,
{
    let state = store.state.get();

    match &action.action {
        Action::StorageBlocksGenesisInitHeaderPutInit(content) => {
            let config = &state.config;
            let chain_id = config.chain_id.clone();
            let genesis_hash = config.init_storage_data.genesis_block_header_hash.clone();
            let genesis_header = config
                .protocol_runner
                .environment
                .genesis_header(
                    content.genesis_commit_hash.clone(),
                    get_empty_operation_list_list_hash().unwrap(),
                )
                .unwrap();

            let block_header_with_hash = BlockHeaderWithHash {
                hash: genesis_hash,
                header: Arc::new(genesis_header),
            };
            store.dispatch(StorageRequestCreateAction {
                payload: StorageRequestPayload::BlockHeaderPut(chain_id, block_header_with_hash),
            });
            store.dispatch(StorageBlocksGenesisInitHeaderPutPendingAction {});
        }
        Action::StorageRequestError(content) => {
            let req_id = match &state.storage.blocks.genesis.init {
                StorageBlocksGenesisInitState::HeaderPut(
                    StorageBlocksGenesisInitHeaderPutState::Pending { req_id },
                ) => *req_id,
                _ => return,
            };
            if req_id == content.req_id {
                let _ = match &content.error {
                    StorageResponseError::BlockHeaderPutError(err) => err.clone(),
                    _ => return,
                };
                store.dispatch(StorageBlocksGenesisInitHeaderPutErrorAction {});
            }
        }
        Action::StorageBlocksGenesisInitHeaderPutError(_) => {
            todo!("handle error when saving genesis block headers to storage.");
        }
        Action::StorageRequestSuccess(content) => {
            let req_id = match &state.storage.blocks.genesis.init {
                StorageBlocksGenesisInitState::HeaderPut(
                    StorageBlocksGenesisInitHeaderPutState::Pending { req_id },
                ) => *req_id,
                _ => return,
            };
            if req_id == content.req_id {
                let is_new_block = match &content.result {
                    StorageResponseSuccess::BlockHeaderPutSuccess(v) => *v,
                    _ => return,
                };
                store.dispatch(StorageBlocksGenesisInitHeaderPutSuccessAction { is_new_block });
            }
        }
        _ => {}
    }
}
