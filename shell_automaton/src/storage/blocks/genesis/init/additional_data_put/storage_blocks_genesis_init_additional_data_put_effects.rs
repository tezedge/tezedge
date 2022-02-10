// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use storage::BlockAdditionalData;

use crate::service::storage_service::{
    StorageRequestPayload, StorageResponseError, StorageResponseSuccess,
};
use crate::storage::blocks::genesis::init::StorageBlocksGenesisInitState;
use crate::storage::request::{StorageRequestCreateAction, StorageRequestor};
use crate::{Action, ActionWithMeta, Service, Store};

use super::{
    StorageBlocksGenesisInitAdditionalDataPutErrorAction,
    StorageBlocksGenesisInitAdditionalDataPutPendingAction,
    StorageBlocksGenesisInitAdditionalDataPutState,
    StorageBlocksGenesisInitAdditionalDataPutSuccessAction,
};

pub fn storage_blocks_genesis_init_additional_data_put_effects<S>(
    store: &mut Store<S>,
    action: &ActionWithMeta,
) where
    S: Service,
{
    let state = store.state.get();

    match &action.action {
        Action::StorageBlocksGenesisInitAdditionalDataPutInit(_) => {
            let config = &state.config;
            let genesis_hash = config.init_storage_data.genesis_block_header_hash.clone();
            let genesis_additional_data = config
                .protocol_runner
                .environment
                .genesis_additional_data()
                .unwrap();
            let block_additional_data = BlockAdditionalData::new(
                genesis_additional_data.max_operations_ttl,
                genesis_additional_data.last_allowed_fork_level,
                genesis_additional_data.protocol_hash,
                genesis_additional_data.next_protocol_hash,
                None,
                None,
                None,
            );

            store.dispatch(StorageRequestCreateAction {
                payload: StorageRequestPayload::BlockAdditionalDataPut((
                    genesis_hash,
                    block_additional_data,
                )),
                requestor: StorageRequestor::None,
            });
            store.dispatch(StorageBlocksGenesisInitAdditionalDataPutPendingAction {});
        }
        Action::StorageRequestError(content) => {
            let req_id = match &state.storage.blocks.genesis.init {
                StorageBlocksGenesisInitState::AdditionalDataPut(
                    StorageBlocksGenesisInitAdditionalDataPutState::Pending { req_id },
                ) => *req_id,
                _ => return,
            };
            if req_id == content.req_id {
                let _ = match &content.error {
                    StorageResponseError::BlockAdditionalDataPutError(err) => err.clone(),
                    _ => return,
                };
                store.dispatch(StorageBlocksGenesisInitAdditionalDataPutErrorAction {});
            }
        }
        Action::StorageBlocksGenesisInitAdditionalDataPutError(_) => {
            todo!("handle error when putting additional data for genesis block to storage");
        }
        Action::StorageRequestSuccess(content) => {
            let req_id = match &state.storage.blocks.genesis.init {
                StorageBlocksGenesisInitState::AdditionalDataPut(
                    StorageBlocksGenesisInitAdditionalDataPutState::Pending { req_id },
                ) => *req_id,
                _ => return,
            };
            if req_id == content.req_id {
                match &content.result {
                    StorageResponseSuccess::BlockAdditionalDataPutSuccess(_) => {}
                    _ => return,
                };
                store.dispatch(StorageBlocksGenesisInitAdditionalDataPutSuccessAction {});
            }
        }
        _ => {}
    }
}
