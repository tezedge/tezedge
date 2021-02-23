// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::merkle_storage::{ContextValue, EntryHash};

use crate::persistent::database::{ SimpleKeyValueStoreWithSchema, DBError};
use crate::MerkleStorage;
use rocksdb::{WriteOptions, DB};
use serde::{Deserialize, Serialize};
use std::ops::Deref;
use std::sync::Arc;

pub struct RocksDBBackend {
    inner: Arc<DB>,
}

impl RocksDBBackend {
    pub fn new(db: Arc<DB>) -> Self {
        RocksDBBackend {
            inner: db,
        }
    }
}

impl RocksDBBackend {
    fn default_write_options() -> WriteOptions {
        let mut opts = WriteOptions::default();
        opts.set_sync(false);
        opts
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RocksDBBackendStats {
    mem_table_total: u64,
    mem_table_unflushed: u64,
    mem_table_readers_total: u64,
    cache_total: u64,
}

impl SimpleKeyValueStoreWithSchema<MerkleStorage> for RocksDBBackend {
    fn is_persistent(&self) -> bool {
        (self.inner.deref() as & dyn SimpleKeyValueStoreWithSchema::<MerkleStorage>).is_persistent()
    }

    fn put(& self, key: &EntryHash, value: &ContextValue) -> Result<(), DBError> {
        (self.inner.deref() as & dyn SimpleKeyValueStoreWithSchema::<MerkleStorage>).put(key,value)
    }

    fn delete(&self, key: &EntryHash) -> Result<(), DBError> {
        (self.inner.deref() as & dyn SimpleKeyValueStoreWithSchema::<MerkleStorage>).delete(key)
    }

    fn merge(&self, key: &EntryHash, value: &ContextValue) -> Result<(), DBError> {
        (self.inner.deref() as & dyn SimpleKeyValueStoreWithSchema::<MerkleStorage>).merge(key, value)
    }

    fn get(&self, key: &EntryHash) -> Result<Option<ContextValue>, DBError> {
        (self.inner.deref() as & dyn SimpleKeyValueStoreWithSchema::<MerkleStorage>).get(key)
    }

    fn contains(&self, key: &EntryHash) -> Result<bool, DBError> {
        (self.inner.deref() as & dyn SimpleKeyValueStoreWithSchema::<MerkleStorage>).contains(key)
    }

    fn retain(&self, predicate: &dyn Fn(&EntryHash) -> bool) -> Result<(), DBError> {
        (self.inner.deref() as & dyn SimpleKeyValueStoreWithSchema::<MerkleStorage>).retain(predicate)
    }

    fn write_batch(&self, batch: Vec<(EntryHash, ContextValue)> ) -> Result<(), DBError> {
        (self.inner.deref() as & dyn SimpleKeyValueStoreWithSchema::<MerkleStorage>).write_batch(batch)
    }

    fn total_get_mem_usage(&self) -> Result<usize,DBError> {
        (self.inner.deref() as & dyn SimpleKeyValueStoreWithSchema::<MerkleStorage>).total_get_mem_usage()
    }
}
