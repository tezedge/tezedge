use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use log::trace;

use tezos_encoding::hash::{HashRef, ToHashRef, ChainId, BlockHash};

use crate::{BlockHeaderWithHash, StorageError};
use crate::block_storage::{BlockStorage, BlockStorageDatabase};

pub struct BlockState {
    storage: BlockStorage,
    missing_blocks: HashSet<HashRef>,
    genesis: HashRef,
    current_head: Arc<Mutex<HashRef>>,
    current_chain_id: HashRef,
}

impl BlockState {
    pub fn new(db: Arc<BlockStorageDatabase>, chain_id: ChainId, genesis: BlockHash, current_head: BlockHash) -> Self {
        BlockState {
            storage: BlockStorage::new(db),
            missing_blocks: HashSet::new(),
            genesis: genesis.to_hash_ref(),
            current_head: Arc::new(Mutex::new(current_head.to_hash_ref())),
            current_chain_id: chain_id.to_hash_ref()
        }
    }

    pub fn insert_block_header(&mut self, block_header: BlockHeaderWithHash) -> Result<(), StorageError> {
        let predecessor_block_hash = (&block_header.header.predecessor).to_hash_ref();
        // check if we already have seen predecessor
        self.schedule_block_hash(predecessor_block_hash)?;

        // remove from missing blocks
        self.missing_blocks.remove(&block_header.hash);
        // store block
        self.storage.insert(block_header)
    }

    pub fn schedule_block_hash(&mut self, block_hash: HashRef) -> Result<(), StorageError> {
        if !self.storage.contains(&block_hash)? {
            self.missing_blocks.insert(block_hash);
        } else {
            trace!("Block {:?} is already present in storage", &block_hash);
        }
        Ok(())
    }

    pub fn move_to_queue(&mut self, n: usize) -> Vec<HashRef> {
        self.missing_blocks
            .drain()
            .take(n)
            .collect()
    }

    pub fn has_missing_blocks(&self) -> bool {
        !self.missing_blocks.is_empty()
    }

    pub fn get_current_chain_id(&self) -> ChainId {
        self.current_chain_id.get_hash()
    }

    pub fn get_block(&self, block_hash: &HashRef) -> Option<&BlockHeaderWithHash> {
        self.storage.get(&block_hash)
    }
}