use std::sync::Arc;

use tezos_encoding::hash::HashRef;

use crate::BlockHeaderWithHash;
use crate::persistent::{DatabaseWithSchema, Schema};
use rocksdb::{ColumnFamilyDescriptor, Options};

pub type BlockStorageDatabase = dyn DatabaseWithSchema<BlockStorage> + Sync + Send;

/// Structure for representing in-memory db for - just for demo purposes.
pub struct BlockStorage {
    db: Arc<BlockStorageDatabase>
}

impl BlockStorage {

    pub fn new(db: Arc<BlockStorageDatabase>) -> Self {
        BlockStorage { db }
    }

    pub fn insert(&mut self, block: BlockHeaderWithHash) {
        self.db.put(&block.hash, &block).unwrap();
    }

    pub fn get(&self, block_hash: &HashRef) -> Option<BlockHeaderWithHash> {
        self.db.get(block_hash).unwrap()
    }

    pub fn contains(&self, block_hash: &HashRef) -> bool {
        self.get(block_hash).is_some()
    }
}

impl Schema for BlockStorage {
    const COLUMN_FAMILY_NAME: &'static str = "block_storage";
    type Key = HashRef;
    type Value = BlockHeaderWithHash;

    fn cf_descriptor() -> ColumnFamilyDescriptor {
        ColumnFamilyDescriptor::new(Self::COLUMN_FAMILY_NAME, Options::default())
    }
}
