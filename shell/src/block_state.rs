// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::cmp;
use std::cmp::Ordering;
use std::sync::Arc;

use storage::{BlockHeaderWithHash, BlockMetaStorage, BlockMetaStorageDatabase, BlockStorage, BlockStorageDatabase, BlockStorageReader, IteratorMode, StorageError};
use tezos_encoding::hash::{BlockHash, ChainId};

use crate::collections::{BlockData, UniqueBlockData};

pub struct BlockState {
    block_storage: BlockStorage,
    block_meta_storage: BlockMetaStorage,
    missing_blocks: UniqueBlockData<MissingBlock>,
    chain_id: ChainId,
}

impl BlockState {
    pub fn new(db: Arc<BlockStorageDatabase>, meta_db: Arc<BlockMetaStorageDatabase>, chain_id: &ChainId) -> Self {
        BlockState {
            block_storage: BlockStorage::new(db),
            block_meta_storage: BlockMetaStorage::new(meta_db),
            missing_blocks: UniqueBlockData::new(),
            chain_id: chain_id.clone()
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
        self.block_meta_storage.put_block_header(block_header)?;

        Ok(())
    }

    #[inline]
    pub fn drain_missing_blocks(&mut self, n: usize) -> Vec<MissingBlock> {
        (0..cmp::min(self.missing_blocks.len(), n))
            .map(|_| self.missing_blocks.pop().unwrap())
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
            let (key, value) = (key?, value?);
            if value.predecessor.is_none() {
                self.missing_blocks.push(MissingBlock {
                    block_hash: key,
                    level: value.level
                });
            }
        }

        Ok(())
    }

    #[inline]
    pub fn get_chain_id(&self) -> &ChainId {
        &self.chain_id
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
