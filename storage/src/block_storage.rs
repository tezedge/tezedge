use std::collections::HashMap;

use tezos_encoding::hash::HashRef;

use crate::BlockHeaderWithHash;

/// Structure for representing in-memory db for - just for demo purposes.
pub struct BlockStorage(HashMap<HashRef, BlockHeaderWithHash>);

impl BlockStorage {

    pub fn new() -> Self {
        BlockStorage(HashMap::new())
    }

    pub fn insert(&mut self, block: BlockHeaderWithHash) {
        self.0.insert(block.hash.clone(), block);
    }

    pub fn get(&self, block_hash: &HashRef) -> Option<&BlockHeaderWithHash> {
        self.0.get(block_hash)
    }

    pub fn is_present(&self, block_hash: &HashRef) -> bool {
        self.0.contains_key(block_hash)
    }
}

