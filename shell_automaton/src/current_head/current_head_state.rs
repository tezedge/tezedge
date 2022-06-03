// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crypto::hash::{BlockHash, BlockMetadataHash, BlockPayloadHash, OperationMetadataListListHash};
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

        payload_hash: Option<BlockPayloadHash>,
        // Needed for mempool prevalidator's begin construction
        // for prevalidation request.
        block_metadata_hash: Option<BlockMetadataHash>,
        ops_metadata_hash: Option<OperationMetadataListListHash>,

        /// Stores all applied blocks on last 2 levels.
        applied_blocks: BTreeMap<BlockHash, BlockHeaderWithHash>,
    },
}

impl CurrentHeadState {
    #[inline(always)]
    pub fn new() -> Self {
        Self::Idle
    }

    pub fn rehydrated(head: BlockHeaderWithHash, head_pred: Option<BlockHeaderWithHash>) -> Self {
        let applied_blocks: BTreeMap<BlockHash, _> = IntoIterator::into_iter([
            Some((head.hash.clone(), head.clone())),
            head_pred
                .as_ref()
                .map(|pred| (pred.hash.clone(), pred.clone())),
        ])
        .flatten()
        .filter(|(_, b)| b.header.level() + 1 >= head.header.level())
        .collect();

        Self::Rehydrated {
            head,
            head_pred,
            payload_hash: None,
            block_metadata_hash: None,
            ops_metadata_hash: None,
            applied_blocks,
        }
    }

    pub fn set_block_metadata_hash(&mut self, value: Option<BlockMetadataHash>) -> &mut Self {
        if let Self::Rehydrated {
            block_metadata_hash,
            ..
        } = self
        {
            *block_metadata_hash = value;
        }
        self
    }

    pub fn set_ops_metadata_hash(
        &mut self,
        value: Option<OperationMetadataListListHash>,
    ) -> &mut Self {
        if let Self::Rehydrated {
            ops_metadata_hash, ..
        } = self
        {
            *ops_metadata_hash = value;
        }
        self
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

    pub fn payload_hash(&self) -> Option<&BlockPayloadHash> {
        match self {
            Self::Rehydrated { payload_hash, .. } => payload_hash.as_ref(),
            _ => None,
        }
    }

    pub fn is_applied(&self, block_hash: &BlockHash) -> bool {
        match self {
            Self::Rehydrated { applied_blocks, .. } => applied_blocks.contains_key(block_hash),
            _ => false,
        }
    }
}

impl Default for CurrentHeadState {
    fn default() -> Self {
        Self::new()
    }
}
