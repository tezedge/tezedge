use std::collections::HashMap;

use networking::p2p::encoding::prelude::*;
use networking::p2p::binary_message::{MessageHash, MessageHashError};
use tezos_encoding::hash::BlockHash;

/// Structure for representing in-memory db for - just for demo purposes.
pub struct BlockStorage(HashMap<BlockHash, BlockHeader>);

impl BlockStorage {

    pub fn new() -> Self {
        BlockStorage(HashMap::new())
    }

    pub fn insert(&mut self, block: BlockHeader) -> Result<BlockHash, MessageHashError> {
        let key = block.message_hash()?;
        self.0.insert(key.clone(), block);
        Ok(key)
    }

    pub fn get(&self, block_hash: &[u8]) -> Option<&BlockHeader> {
        self.0.get(block_hash)
    }

    pub fn is_present(&self, block_hash: &[u8]) -> bool {
        self.0.contains_key(block_hash)
    }
}

