// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use storage::BlockHeaderWithHash;

use crate::protocol_runner::ProtocolRunnerState;
use crate::request::RequestId;
use crate::service::storage_service::StorageError;
use crate::storage::blocks::genesis::init::StorageBlocksGenesisInitState;
use crate::{EnablingCondition, State};

use super::CurrentHeadState;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CurrentHeadRehydrateInitAction {}

impl EnablingCondition<State> for CurrentHeadRehydrateInitAction {
    fn is_enabled(&self, state: &State) -> bool {
        if !matches!(&state.current_head, CurrentHeadState::Idle) {
            return false;
        }
        if let ProtocolRunnerState::Ready(protocol) = &state.protocol_runner {
            protocol.genesis_commit_hash.is_none()
                || matches!(
                    &state.storage.blocks.genesis.init,
                    StorageBlocksGenesisInitState::Success
                )
        } else {
            false
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CurrentHeadRehydratePendingAction {
    pub storage_req_id: RequestId,
}

impl EnablingCondition<State> for CurrentHeadRehydratePendingAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.current_head {
            CurrentHeadState::RehydrateInit { .. } => true,
            _ => false,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CurrentHeadRehydrateErrorAction {
    pub error: StorageError,
}

impl EnablingCondition<State> for CurrentHeadRehydrateErrorAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.current_head {
            CurrentHeadState::RehydratePending { .. } => true,
            _ => false,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CurrentHeadRehydrateSuccessAction {
    pub head: BlockHeaderWithHash,
}

impl EnablingCondition<State> for CurrentHeadRehydrateSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.current_head {
            CurrentHeadState::RehydratePending { .. } => true,
            _ => false,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CurrentHeadRehydratedAction {}

impl EnablingCondition<State> for CurrentHeadRehydratedAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.current_head {
            CurrentHeadState::RehydrateSuccess { .. } => true,
            _ => false,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CurrentHeadUpdateAction {
    pub new_head: BlockHeaderWithHash,
}

impl EnablingCondition<State> for CurrentHeadUpdateAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.current_head {
            CurrentHeadState::Rehydrated { head } => true,
            _ => false,
        }
    }
}
