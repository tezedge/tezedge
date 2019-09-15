use std::sync::Arc;

use tezos_encoding::hash::{HashRef, HashType};

use crate::BlockHeaderWithHash;
use crate::persistent::{DatabaseWithSchema, Schema};
use rocksdb::{ColumnFamilyDescriptor, Options, SliceTransform};

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
        let mut cf_opts = Options::default();
        cf_opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(HashType::BlockHash.size()));
        cf_opts.set_memtable_prefix_bloom_ratio(0.1);
        ColumnFamilyDescriptor::new(Self::COLUMN_FAMILY_NAME, cf_opts)
    }
}
