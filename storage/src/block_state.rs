use std::collections::HashSet;

use tezos_encoding::hash::{HashRef, ToHashRef, ChainId, BlockHash};

use crate::block_storage::BlockStorage;
use crate::BlockHeaderWithHash;
use std::sync::{Arc, Mutex};

pub struct BlockState {
    storage: BlockStorage,
    missing_blocks: HashSet<HashRef>,
    current_head: Arc<Mutex<HashRef>>,
    current_chain_id: HashRef,
}

impl BlockState {
    pub fn new(chain_id: ChainId, current_head: BlockHash) -> Self {
        BlockState {
            storage: BlockStorage::new(),
            missing_blocks: HashSet::new(),
            current_head: Arc::new(Mutex::new(current_head.to_hash_ref())),
            current_chain_id: chain_id.to_hash_ref()
        }
    }

    pub fn insert_block_header(&mut self, block_header: BlockHeaderWithHash) {
        let predecessor_block_hash = (&block_header.header.predecessor).to_hash_ref();
        // check if we already have seen predecessor
        if !self.storage.is_present(&predecessor_block_hash) {
            // block was not seen before
            self.missing_blocks.insert(predecessor_block_hash);
        }
        // remove from missing blocks
        self.missing_blocks.remove(&block_header.hash);
        // store block
        self.storage.insert(block_header);
    }

    pub fn schedule_block_hash(&mut self, block_hash: HashRef) {
        if !self.storage.is_present(&block_hash) {
            self.missing_blocks.insert(block_hash);
        }
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
}