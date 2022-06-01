// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use crypto::hash::{
    BlockMetadataHash, BlockPayloadHash, OperationMetadataListListHash, ProtocolHash,
};
use storage::BlockHeaderWithHash;
use tezos_messages::p2p::encoding::operation::Operation;

use crate::protocol_runner::ProtocolRunnerState;
use crate::request::RequestId;
use crate::service::storage_service::StorageError;
use crate::storage::blocks::genesis::init::StorageBlocksGenesisInitState;
use crate::{EnablingCondition, State};

use super::{CurrentHeadState, ProtocolConstants};

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
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

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CurrentHeadRehydratePendingAction {
    pub storage_req_id: RequestId,
}

impl EnablingCondition<State> for CurrentHeadRehydratePendingAction {
    fn is_enabled(&self, state: &State) -> bool {
        matches!(&state.current_head, CurrentHeadState::RehydrateInit { .. })
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CurrentHeadRehydrateErrorAction {
    pub error: StorageError,
}

impl EnablingCondition<State> for CurrentHeadRehydrateErrorAction {
    fn is_enabled(&self, state: &State) -> bool {
        matches!(
            &state.current_head,
            CurrentHeadState::RehydratePending { .. }
        )
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CurrentHeadRehydrateSuccessAction {
    pub head: BlockHeaderWithHash,
    pub head_pred: Option<BlockHeaderWithHash>,

    pub block_metadata_hash: Option<BlockMetadataHash>,
    pub ops_metadata_hash: Option<OperationMetadataListListHash>,

    pub pred_block_metadata_hash: Option<BlockMetadataHash>,
    pub pred_ops_metadata_hash: Option<OperationMetadataListListHash>,

    pub operations: Vec<Vec<Operation>>,
    pub constants: Option<ProtocolConstants>,
}

impl EnablingCondition<State> for CurrentHeadRehydrateSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        matches!(
            &state.current_head,
            CurrentHeadState::RehydratePending { .. }
        )
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CurrentHeadRehydratedAction {}

impl EnablingCondition<State> for CurrentHeadRehydratedAction {
    fn is_enabled(&self, state: &State) -> bool {
        matches!(
            &state.current_head,
            CurrentHeadState::RehydrateSuccess { .. }
        )
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CurrentHeadUpdateAction {
    pub new_head: BlockHeaderWithHash,
    pub protocol: ProtocolHash,
    pub next_protocol: ProtocolHash,
    pub payload_hash: Option<BlockPayloadHash>,
    pub block_metadata_hash: Option<BlockMetadataHash>,
    pub ops_metadata_hash: Option<OperationMetadataListListHash>,
    pub pred_block_metadata_hash: Option<BlockMetadataHash>,
    pub pred_ops_metadata_hash: Option<OperationMetadataListListHash>,
    pub operations: Vec<Vec<Operation>>,
    pub new_constants: Option<ProtocolConstants>,
}

impl EnablingCondition<State> for CurrentHeadUpdateAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.current_head {
            CurrentHeadState::Rehydrated { head, .. } => {
                self.new_head.header.fitness().gt(head.header.fitness())
            }
            _ => false,
        }
    }
}
