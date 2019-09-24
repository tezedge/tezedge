// Copyright (c) SimpleStaking and Tezos-RS Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use tezos_encoding::hash::{BlockHash, ChainId};

use crate::{BlockHeaderWithHash, StorageError};
use crate::block_meta_storage::{BlockMetaStorage, BlockMetaStorageDatabase};
use crate::block_storage::{BlockStorage, BlockStorageDatabase, BlockStorageReader};
use crate::persistent::database::IteratorMode;

pub struct BlockState {
    block_storage: BlockStorage,
    meta_storage: BlockMetaStorage,
    missing_blocks: Vec<BlockHash>,
    chain_id: ChainId,
}

impl BlockState {
    pub fn new(db: Arc<BlockStorageDatabase>, meta_db: Arc<BlockMetaStorageDatabase>, chain_id: &ChainId) -> Self {
        BlockState {
            block_storage: BlockStorage::new(db),
            meta_storage: BlockMetaStorage::new(meta_db),
            missing_blocks: Vec::new(),
            chain_id: chain_id.clone()
        }
    }

    pub fn process_block_header(&mut self, block_header: BlockHeaderWithHash) -> Result<(), StorageError> {
        // check if we already have seen predecessor
        self.push_missing_block(block_header.header.predecessor.clone())?;

        // store block
        self.block_storage.put_block_header(&block_header)?;
        // update meta
        self.meta_storage.put_block_header(&block_header)?;

        Ok(())
    }

    pub fn push_missing_block(&mut self, block_hash: BlockHash) -> Result<(), StorageError> {
        if !self.block_storage.contains(&block_hash)? {
            self.missing_blocks.push(block_hash);
        }
        Ok(())
    }

    pub fn drain_missing_blocks(&mut self, n: usize) -> Vec<BlockHash> {
        self.missing_blocks
            .drain(..n)
            .collect()
    }

    pub fn has_missing_blocks(&self) -> bool {
        !self.missing_blocks.is_empty()
    }

    pub fn hydrate(&mut self) -> Result<(), StorageError> {
        for (key, value) in self.meta_storage.iter(IteratorMode::Start)? {
            if value?.predecessor.is_none() {
                self.missing_blocks.push(key?);
            }
        }

        Ok(())
    }

    pub fn get_chain_id(&self) -> &ChainId {
        &self.chain_id
    }
}