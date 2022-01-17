// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::{Action, ActionWithMeta, State};

use super::StorageBlocksGenesisInitAdditionalDataPutState;

pub fn storage_blocks_genesis_init_additional_data_put_reducer(
    state: &mut State,
    action: &ActionWithMeta,
) {
    match &action.action {
        Action::StorageBlocksGenesisInitAdditionalDataPutInit(_) => {
            state.storage.blocks.genesis.init =
                StorageBlocksGenesisInitAdditionalDataPutState::Init {}.into();
        }
        Action::StorageBlocksGenesisInitAdditionalDataPutPending(_) => {
            let req_id = state.storage.requests.last_added_req_id();
            state.storage.blocks.genesis.init =
                StorageBlocksGenesisInitAdditionalDataPutState::Pending { req_id }.into();
        }
        Action::StorageBlocksGenesisInitAdditionalDataPutError(_) => {
            state.storage.blocks.genesis.init =
                StorageBlocksGenesisInitAdditionalDataPutState::Error {}.into();
        }
        Action::StorageBlocksGenesisInitAdditionalDataPutSuccess(_) => {
            state.storage.blocks.genesis.init =
                StorageBlocksGenesisInitAdditionalDataPutState::Success {}.into();
        }
        _ => {}
    }
}
