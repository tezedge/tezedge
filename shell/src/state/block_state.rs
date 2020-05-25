// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::cmp;
use std::cmp::Ordering;

use rand::Rng;

use crypto::hash::{BlockHash, ChainId};
use storage::{BlockHeaderWithHash, BlockMetaStorage, BlockStorage, BlockStorageReader, IteratorMode, StorageError};
use storage::persistent::PersistentStorage;

use crate::collections::{BlockData, UniqueBlockData};

/// Holds state of all known blocks
pub struct BlockchainState {
    /// persistent block storage
    block_storage: BlockStorage,
    ///persistent block metadata storage
    block_meta_storage: BlockMetaStorage,
    /// Current missing blocks.
    /// This represents a set of missing block we will try to retrieve in the future.
    /// Before we try to fetch missing block it is removed from this queue.
    /// Block is then sent to [`chain_manager`](crate::chain_manager::ChainManager) actor whose responsibility is to
    /// retrieve the block data. If the block data cannot be fetched it's the responsibility
    /// of the [`chain_manager`](crate::chain_manager::ChainManager) to return the block to this queue.
    missing_blocks: UniqueBlockData<MissingBlock>,
    chain_id: ChainId,
}

impl BlockchainState {
    pub fn new(persistent_storage: &PersistentStorage, chain_id: &ChainId) -> Self {
        BlockchainState {
            block_storage: BlockStorage::new(persistent_storage),
            block_meta_storage: BlockMetaStorage::new(persistent_storage),
            missing_blocks: UniqueBlockData::new(),
            chain_id: chain_id.clone(),
        }
    }

    pub fn process_block_header(&mut self, block_header: &BlockHeaderWithHash) -> Result<(), StorageError> {
        // check if we already have seen predecessor
        self.push_missing_block(MissingBlock {
            block_hash: block_header.header.predecessor().clone(),
            level: block_header.header.level() - 1
        })?;

        // store block
        self.block_storage.put_block_header(block_header)?;
        // update meta
        self.block_meta_storage.put_block_header(block_header, &self.chain_id)?;

        Ok(())
    }

    #[inline]
    pub fn drain_missing_blocks(&mut self, n: usize, level_max: i32) -> Vec<MissingBlock> {
        (0..cmp::min(self.missing_blocks.len(), n))
            .filter_map(|_| {
                if self.missing_blocks.peek().filter(|block| block.level <= level_max).is_some() {
                    self.missing_blocks.pop()
                } else {
                    None
                }
            })
            .collect()
    }

    #[inline]
    pub fn push_missing_block(&mut self, missing_block: MissingBlock) -> Result<(), StorageError> {
        if !self.block_storage.contains(&missing_block.block_hash)? {
            self.missing_blocks.push(missing_block);
        }
        Ok(())
    }

    #[inline]
    pub fn has_missing_blocks(&self) -> bool {
        !self.missing_blocks.is_empty()
    }

    pub fn hydrate(&mut self) -> Result<(), StorageError> {
        for (key, value) in self.block_meta_storage.iter(IteratorMode::Start)? {
            let (block_hash, meta) = (key?, value?);
            if meta.predecessor().is_none() && (meta.chain_id() == &self.chain_id) {
                self.missing_blocks.push(MissingBlock {
                    block_hash,
                    level: meta.level(),
                });
            }
        }

        Ok(())
    }

    #[inline]
    pub fn get_chain_id(&self) -> &ChainId {
        &self.chain_id
    }

    pub fn get_history(&self) -> Result<Vec<BlockHash>, StorageError> {
        let history_max = 20;
        let mut history = Vec::with_capacity(history_max);
        let mut rng = rand::thread_rng();
        for (key, value) in self.block_meta_storage.iter(IteratorMode::Start)? {
            let pivot = (1 + rng.gen::<u8>() % 24) as i32;
            let (block_hash, meta) = (key?, value?);
            if meta.is_applied() && (meta.level() != 0) && (meta.level() % pivot == 0) && (meta.chain_id() == &self.chain_id) {
                history.push(block_hash);
                if history.len() >= history_max {
                    break;
                }
            }
        }
        Ok(history)
    }
}

#[derive(Clone, Debug)]
pub struct MissingBlock {
    pub block_hash: BlockHash,
    pub level: i32
}

impl BlockData for MissingBlock {
    #[inline]
    fn block_hash(&self) -> &BlockHash {
        &self.block_hash
    }
}

impl From<BlockHash> for MissingBlock {
    fn from(block_hash: BlockHash) -> Self {
        MissingBlock {
            block_hash,
            //TODO: refactor to support None
            level: 0
        }
    }
}

impl PartialEq for MissingBlock {
    fn eq(&self, other: &Self) -> bool {
        self.level == other.level && self.block_hash == other.block_hash
    }
}

impl Eq for MissingBlock {}

impl PartialOrd for MissingBlock {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MissingBlock {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.level, &self.block_hash).cmp(&(other.level, &other.block_hash)).reverse()
    }
}
