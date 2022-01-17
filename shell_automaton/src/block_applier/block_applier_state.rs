// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use storage::block_meta_storage::Meta;
use storage::{BlockAdditionalData, BlockHeaderWithHash};
use tezos_api::ffi::{ApplyBlockRequest, ApplyBlockResponse};

use crypto::hash::{BlockHash, ChainId};

use crate::request::RequestId;
use crate::service::storage_service::StorageError;
use crate::Config;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum BlockApplierApplyError {
    PrepareData(StorageError),
    BlockHeaderNotFound,
    PredecessorBlockHeaderGet(StorageError),
    PredecessorBlockHeaderNotFound,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum BlockApplierApplyState {
    Idle {
        time: u64,
    },
    Init {
        time: u64,
        chain_id: Arc<ChainId>,
        block_hash: Arc<BlockHash>,
    },

    PrepareDataPending {
        time: u64,
        storage_req_id: RequestId,
        chain_id: Arc<ChainId>,
        block_hash: Arc<BlockHash>,
    },
    PrepareDataSuccess {
        time: u64,
        prepare_data_duration: u64,
        chain_id: Arc<ChainId>,
        block: Arc<BlockHeaderWithHash>,
        block_meta: Arc<Meta>,
        apply_block_req: Arc<ApplyBlockRequest>,
    },

    ProtocolRunnerApplyPending {
        time: u64,
        prepare_data_duration: u64,
        chain_id: Arc<ChainId>,
        block: Arc<BlockHeaderWithHash>,
        block_meta: Arc<Meta>,
        apply_block_req: Arc<ApplyBlockRequest>,
    },
    ProtocolRunnerApplySuccess {
        time: u64,
        prepare_data_duration: u64,
        protocol_runner_apply_duration: u64,
        chain_id: Arc<ChainId>,
        block: Arc<BlockHeaderWithHash>,
        block_meta: Arc<Meta>,
        apply_result: Arc<ApplyBlockResponse>,
    },

    StoreApplyResultPending {
        time: u64,
        prepare_data_duration: u64,
        protocol_runner_apply_duration: u64,
        storage_req_id: RequestId,
        chain_id: Arc<ChainId>,
        block: Arc<BlockHeaderWithHash>,
        block_meta: Arc<Meta>,
        apply_result: Arc<ApplyBlockResponse>,
    },
    StoreApplyResultSuccess {
        time: u64,
        prepare_data_duration: u64,
        protocol_runner_apply_duration: u64,
        store_apply_result_duration: u64,
        chain_id: Arc<ChainId>,
        block: Arc<BlockHeaderWithHash>,
        block_additional_data: Arc<BlockAdditionalData>,
        apply_result: Arc<ApplyBlockResponse>,
    },

    Error {
        error: BlockApplierApplyError,
        chain_id: Arc<ChainId>,
        block_hash: Arc<BlockHash>,
    },
    Success {
        time: u64,
        prepare_data_duration: u64,
        protocol_runner_apply_duration: u64,
        store_apply_result_duration: u64,
        chain_id: Arc<ChainId>,
        block: Arc<BlockHeaderWithHash>,
        block_additional_data: Arc<BlockAdditionalData>,
        apply_result: Arc<ApplyBlockResponse>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlockApplierState {
    pub queue: VecDeque<(Arc<ChainId>, Arc<BlockHash>)>,
    pub current: BlockApplierApplyState,
    /// Used as a cache for predecessor block header.
    ///
    /// Might not be latest applied block as we don't read this value
    /// from storage. TODO: read it from storage.
    pub last_applied: Arc<BlockHash>,
}

impl BlockApplierState {
    #[inline(always)]
    pub fn new(config: &Config) -> Self {
        let genesis_hash = config.init_storage_data.genesis_block_header_hash.clone();

        Self {
            queue: VecDeque::new(),
            current: BlockApplierApplyState::Idle { time: 0 },
            last_applied: genesis_hash.into(),
        }
    }
}
