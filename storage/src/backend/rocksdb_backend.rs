// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::merkle_storage::{ContextValue, EntryHash};

use crate::persistent::database::{DBError, KeyValueStoreBackend};
use crate::storage_backend::{GarbageCollector, StorageBackendError};
use crate::MerkleStorage;
use rocksdb::DB;
use serde::{Deserialize, Serialize};
use std::ops::Deref;
use std::sync::Arc;

pub struct RocksDBBackend {
    inner: Arc<DB>,
}

impl RocksDBBackend {
    pub fn new(db: Arc<DB>) -> Self {
        RocksDBBackend { inner: db }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RocksDBBackendStats {
    mem_table_total: u64,
    mem_table_unflushed: u64,
    mem_table_readers_total: u64,
    cache_total: u64,
}

impl GarbageCollector for RocksDBBackend {
    fn new_cycle_started(&mut self) -> Result<(), StorageBackendError> {
        Ok(())
    }

    fn mark_reused(
        &mut self,
        _reused_keys: std::collections::HashSet<EntryHash>,
    ) -> Result<(), StorageBackendError> {
        Ok(())
    }
}

impl KeyValueStoreBackend<MerkleStorage> for RocksDBBackend {
    fn is_persistent(&self) -> bool {
        (self.inner.deref() as &dyn KeyValueStoreBackend<MerkleStorage>).is_persistent()
    }

    fn put(&self, key: &EntryHash, value: &ContextValue) -> Result<(), DBError> {
        (self.inner.deref() as &dyn KeyValueStoreBackend<MerkleStorage>).put(key, value)
    }

    fn delete(&self, key: &EntryHash) -> Result<(), DBError> {
        (self.inner.deref() as &dyn KeyValueStoreBackend<MerkleStorage>).delete(key)
    }

    fn merge(&self, key: &EntryHash, value: &ContextValue) -> Result<(), DBError> {
        (self.inner.deref() as &dyn KeyValueStoreBackend<MerkleStorage>).merge(key, value)
    }

    fn get(&self, key: &EntryHash) -> Result<Option<ContextValue>, DBError> {
        (self.inner.deref() as &dyn KeyValueStoreBackend<MerkleStorage>).get(key)
    }

    fn contains(&self, key: &EntryHash) -> Result<bool, DBError> {
        (self.inner.deref() as &dyn KeyValueStoreBackend<MerkleStorage>).contains(key)
    }

    fn retain(&self, predicate: &dyn Fn(&EntryHash) -> bool) -> Result<(), DBError> {
        (self.inner.deref() as &dyn KeyValueStoreBackend<MerkleStorage>).retain(predicate)
    }

    fn write_batch(&self, batch: Vec<(EntryHash, ContextValue)>) -> Result<(), DBError> {
        (self.inner.deref() as &dyn KeyValueStoreBackend<MerkleStorage>).write_batch(batch)
    }

    fn total_get_mem_usage(&self) -> Result<usize, DBError> {
        (self.inner.deref() as &dyn KeyValueStoreBackend<MerkleStorage>).total_get_mem_usage()
    }
}
