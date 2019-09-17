use std::sync::Arc;

use tezos_encoding::hash::HashRef;

use crate::{BlockHeaderWithHash, StorageError};
use crate::persistent::{DatabaseWithSchema, Schema};

pub type BlockStorageDatabase = dyn DatabaseWithSchema<BlockStorage> + Sync + Send;

/// Structure for representing in-memory db for - just for demo purposes.
pub struct BlockStorage {
    db: Arc<BlockStorageDatabase>
}

impl BlockStorage {

    pub fn new(db: Arc<BlockStorageDatabase>) -> Self {
        BlockStorage { db }
    }

    pub fn insert(&mut self, block: BlockHeaderWithHash) -> Result<(), StorageError> {
        self.db.put(&block.hash, &block)
            .map_err(|e| e.into())
    }

    pub fn get(&self, block_hash: &HashRef) -> Result<Option<BlockHeaderWithHash>, StorageError> {
        self.db.get(block_hash)
            .map_err(|e| e.into())
    }

    pub fn contains(&self, block_hash: &HashRef) -> Result<bool, StorageError> {
        self.get(block_hash)
            .map_err(StorageError::from)
            .map(|v| v.is_some())
    }
}

impl Schema for BlockStorage {
    const COLUMN_FAMILY_NAME: &'static str = "block_storage";
    type Key = HashRef;
    type Value = BlockHeaderWithHash;
}
