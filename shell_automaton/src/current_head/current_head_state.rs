// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;
// use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crypto::hash::{BlockHash, BlockMetadataHash, BlockPayloadHash, OperationMetadataListListHash};
use storage::BlockHeaderWithHash;
use tezos_messages::p2p::encoding::block_header::Level;

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
        ///// Live blocks size equals to `operations_max_ttl`, which at
        ///// the moment is last 120 blocks. So this will contain 118 blocks
        ///// last cemented blocks so that
        ///// `cemented_live_blocks` -> `HEAD~1` -> `HEAD` = last 120 blocks.
        /////
        ///// `Set` is used as a container instead of `Vec`, because we
        ///// only need this data in memory for two things:
        ///// 1. Return it in `live_blocks` rpc (this along with the
        /////    `head` and `head_pred`). Blocks in the result must be
        /////    sorted in a lexicographical order. That is exactly how
        /////    `BTreeSet` will sort them and `BTreeSet::iter()` will
        /////    give us already sorted values.
        ///// 2. To check if the operation's branch is included in the
        /////    live blocks.
        //cemented_live_blocks: BTreeSet<BlockHash>,
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
        .filter_map(|v| v)
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

    pub fn get_pred(&self) -> Option<&BlockHeaderWithHash> {
        match self {
            Self::Rehydrated { head_pred, .. } => head_pred.as_ref(),
            _ => None,
        }
    }

    pub fn hash(&self) -> Option<&BlockHash> {
        self.get().map(|v| &v.hash)
    }

    pub fn level(&self) -> Option<Level> {
        self.get().map(|v| v.header.level())
    }

    pub fn payload_hash(&self) -> Option<&BlockPayloadHash> {
        match self {
            Self::Rehydrated { payload_hash, .. } => payload_hash.as_ref(),
            _ => None,
        }
    }

    pub fn round(&self) -> Option<i32> {
        match self {
            Self::Rehydrated { head, .. } => head.header.fitness().round(),
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

// #[derive(Serialize, Deserialize, Debug, Clone)]
// pub struct BlockHashWithLevelOrdByHash {
//     hash: BlockHash,
//     level: Level,
// }

// impl BlockHashWithLevelOrdByHash {
//     pub fn new(hash: BlockHash, level: Level) -> Self {
//         Self { hash, level }
//     }

//     pub fn hash(&self) -> &BlockHash {
//         &self.hash
//     }

//     pub fn level(&self) -> Level {
//         self.level
//     }
// }

// impl Ord for BlockHashWithLevelOrdByHash {
//     fn cmp(&self, other: &Self) -> std::cmp::Ordering {
//         self.hash.cmp(&other.hash)
//     }
// }
