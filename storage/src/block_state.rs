// Copyright (c) SimpleStaking and Tezos-RS Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use log::trace;

use tezos_encoding::hash::{HashRef, ToHashRef, ChainId, BlockHash};

use crate::{BlockHeaderWithHash, StorageError};
use crate::block_meta_storage::{BlockMetaStorage, BlockMetaStorageDatabase};
use crate::block_storage::{BlockStorage, BlockStorageDatabase};
use crate::persistent::database::IteratorMode;

pub struct BlockState {
    block_storage: BlockStorage,
    meta_storage: BlockMetaStorage,
    missing_blocks: HashSet<BlockHash>,
    genesis: HashRef,
    current_head: Arc<Mutex<HashRef>>,
    current_chain_id: HashRef,
}

impl BlockState {
    pub fn new(db: Arc<BlockStorageDatabase>, meta_db: Arc<BlockMetaStorageDatabase>) -> Self {
    pub fn new(db: Arc<BlockStorageDatabase>, chain_id: ChainId, genesis: BlockHash, current_head: BlockHash) -> Self {
        BlockState {
            block_storage: BlockStorage::new(db),
            meta_storage: BlockMetaStorage::new(meta_db),
            missing_blocks: HashSet::new(),
            genesis: genesis.to_hash_ref(),
            current_head: Arc::new(Mutex::new(current_head.to_hash_ref())),
            current_chain_id: chain_id.to_hash_ref()
        }
    }

    pub fn insert_block_header(&mut self, block_header: BlockHeaderWithHash) -> Result<(), StorageError> {
        // check if we already have seen predecessor
        self.schedule_block_hash(block_header.header.predecessor.clone())?;

        // store block
        self.block_storage.insert(&block_header)?;
        // update meta
        self.meta_storage.insert(&block_header)?;
        // remove from missing blocks
        self.missing_blocks.remove(&block_header.hash);

        Ok(())
    }

    pub fn schedule_block_hash(&mut self, block_hash: BlockHash) -> Result<(), StorageError> {
        if !self.block_storage.contains(&block_hash)? {
            self.missing_blocks.insert(block_hash);
        } else {
            trace!("Block {:?} is already present in storage", &block_hash);
        }
        Ok(())
    }

    pub fn move_to_queue(&mut self, n: usize) -> Vec<BlockHash> {
        self.missing_blocks
            .drain()
            .take(n)
            .collect()
    }

    pub fn has_missing_blocks(&self) -> bool {
        !self.missing_blocks.is_empty()
    }

    pub fn hydrate(&mut self) -> Result<(), StorageError> {
        let BlockState { meta_storage, missing_blocks, .. } = self;
        for (key, value) in meta_storage.iter(IteratorMode::Start)? {
            if value?.predecessor.is_none() {
                missing_blocks.insert(key?);
            }
        }

        Ok(())
    }

    pub fn get_current_chain_id(&self) -> ChainId {
        self.current_chain_id.get_hash()
    }
}