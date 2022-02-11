// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::VecDeque;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crypto::hash::BlockHash;
use storage::block_meta_storage::Meta;
use storage::{BlockAdditionalData, BlockHeaderWithHash};
use tezos_api::ffi::{ApplyBlockError, ApplyBlockRequest, ApplyBlockResponse};
use tezos_protocol_ipc_client::ProtocolServiceError;

use crate::request::RequestId;
use crate::service::rpc_service::RpcId;
use crate::service::storage_service::StorageError;
use crate::Config;

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum BlockApplierApplyError {
    PrepareData(StorageError),
    ProtocolRunnerApply(ProtocolServiceError),
    StoreApplyResult(StorageError),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum BlockApplierApplyState {
    Idle {
        time: u64,
    },
    Init {
        time: u64,
        block_hash: Arc<BlockHash>,
        injector_rpc_id: Option<RpcId>,
    },

    PrepareDataPending {
        time: u64,
        storage_req_id: RequestId,
        block_hash: Arc<BlockHash>,
        injector_rpc_id: Option<RpcId>,
    },
    PrepareDataSuccess {
        time: u64,
        prepare_data_duration: u64,
        block: Arc<BlockHeaderWithHash>,
        block_meta: Arc<Meta>,
        apply_block_req: Arc<ApplyBlockRequest>,
        injector_rpc_id: Option<RpcId>,
    },

    ProtocolRunnerApplyPending {
        time: u64,
        prepare_data_duration: u64,
        block: Arc<BlockHeaderWithHash>,
        block_meta: Arc<Meta>,
        apply_block_req: Arc<ApplyBlockRequest>,

        /// Is retry or not and if yes, what is the reason.
        retry: Option<ApplyBlockError>,
        injector_rpc_id: Option<RpcId>,
    },
    ProtocolRunnerApplySuccess {
        time: u64,
        prepare_data_duration: u64,
        protocol_runner_apply_duration: u64,
        block: Arc<BlockHeaderWithHash>,
        block_meta: Arc<Meta>,
        apply_result: Arc<ApplyBlockResponse>,
        injector_rpc_id: Option<RpcId>,
    },

    StoreApplyResultPending {
        time: u64,
        prepare_data_duration: u64,
        protocol_runner_apply_duration: u64,
        storage_req_id: RequestId,
        block: Arc<BlockHeaderWithHash>,
        block_meta: Arc<Meta>,
        apply_result: Arc<ApplyBlockResponse>,
        injector_rpc_id: Option<RpcId>,
    },
    StoreApplyResultSuccess {
        time: u64,
        prepare_data_duration: u64,
        protocol_runner_apply_duration: u64,
        store_apply_result_duration: u64,
        block: Arc<BlockHeaderWithHash>,
        block_additional_data: Arc<BlockAdditionalData>,
        apply_result: Arc<ApplyBlockResponse>,
        injector_rpc_id: Option<RpcId>,
    },

    Error {
        error: BlockApplierApplyError,
        block_hash: Arc<BlockHash>,
        injector_rpc_id: Option<RpcId>,
    },
    Success {
        time: u64,
        prepare_data_duration: u64,
        protocol_runner_apply_duration: u64,
        store_apply_result_duration: u64,
        block: Arc<BlockHeaderWithHash>,
        block_additional_data: Arc<BlockAdditionalData>,
        apply_result: Arc<ApplyBlockResponse>,
        injector_rpc_id: Option<RpcId>,
    },
}

impl BlockApplierApplyState {
    #[inline(always)]
    pub fn block_hash(&self) -> Option<&BlockHash> {
        match self {
            Self::Idle { .. } => None,
            Self::Init { block_hash, .. } => Some(block_hash),

            Self::PrepareDataPending { block_hash, .. } => Some(block_hash),
            Self::PrepareDataSuccess { block, .. } => Some(&block.hash),

            Self::ProtocolRunnerApplyPending { block, .. } => Some(&block.hash),
            Self::ProtocolRunnerApplySuccess { block, .. } => Some(&block.hash),

            Self::StoreApplyResultPending { block, .. } => Some(&block.hash),
            Self::StoreApplyResultSuccess { block, .. } => Some(&block.hash),

            Self::Error { block_hash, .. } => Some(block_hash),
            Self::Success { block, .. } => Some(&block.hash),
        }
    }

    #[inline(always)]
    pub fn injector_rpc_id(&self) -> Option<RpcId> {
        match self {
            Self::Idle { .. } => None,
            Self::Init {
                injector_rpc_id, ..
            } => injector_rpc_id.clone(),

            Self::PrepareDataPending {
                injector_rpc_id, ..
            } => injector_rpc_id.clone(),
            Self::PrepareDataSuccess {
                injector_rpc_id, ..
            } => injector_rpc_id.clone(),

            Self::ProtocolRunnerApplyPending {
                injector_rpc_id, ..
            } => injector_rpc_id.clone(),
            Self::ProtocolRunnerApplySuccess {
                injector_rpc_id, ..
            } => injector_rpc_id.clone(),

            Self::StoreApplyResultPending {
                injector_rpc_id, ..
            } => injector_rpc_id.clone(),
            Self::StoreApplyResultSuccess {
                injector_rpc_id, ..
            } => injector_rpc_id.clone(),

            Self::Error {
                injector_rpc_id, ..
            } => injector_rpc_id.clone(),
            Self::Success {
                injector_rpc_id, ..
            } => injector_rpc_id.clone(),
        }
    }

    #[inline(always)]
    pub fn is_pending(&self) -> bool {
        match self {
            Self::Idle { .. } => false,
            Self::Error { .. } => false,
            Self::Success { .. } => false,
            _ => true,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlockApplierState {
    pub queue: VecDeque<(Arc<BlockHash>, Option<RpcId>)>,
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
