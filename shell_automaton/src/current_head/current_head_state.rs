// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use crypto::hash::{BlockHash, BlockMetadataHash, OperationMetadataListListHash};
use storage::BlockHeaderWithHash;

use crate::request::RequestId;
use crate::service::storage_service::StorageError;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum CurrentHeadState {
    Idle,

    RehydrateInit {
        time: u64,
    },
    RehydratePending {
        time: u64,
        storage_req_id: RequestId,
    },
    RehydrateError {
        time: u64,
        error: StorageError,
    },
    RehydrateSuccess {
        time: u64,
        head: BlockHeaderWithHash,
        head_pred: Option<BlockHeaderWithHash>,

        block_metadata_hash: Option<BlockMetadataHash>,
        ops_metadata_hash: Option<OperationMetadataListListHash>,
    },

    Rehydrated {
        head: BlockHeaderWithHash,
        head_pred: Option<BlockHeaderWithHash>,

        // Needed for mempool prevalidator's begin construction
        // for prevalidation request.
        block_metadata_hash: Option<BlockMetadataHash>,
        ops_metadata_hash: Option<OperationMetadataListListHash>,
    },
}

impl CurrentHeadState {
    #[inline(always)]
    pub fn new() -> Self {
        Self::Idle
    }

    pub fn get(&self) -> Option<&BlockHeaderWithHash> {
        match self {
            Self::Rehydrated { head, .. } => Some(head),
            _ => None,
        }
    }

    pub fn get_hash(&self) -> Option<&BlockHash> {
        self.get().map(|v| &v.hash)
    }

    pub fn get_pred(&self) -> Option<&BlockHeaderWithHash> {
        match self {
            Self::Rehydrated { head_pred, .. } => head_pred.as_ref(),
            _ => None,
        }
    }
}

impl Default for CurrentHeadState {
    fn default() -> Self {
        Self::new()
    }
}
