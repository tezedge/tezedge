// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::merkle_storage::{ContextValue, EntryHash};

use crate::persistent::database::{DBError, KeyValueStoreBackend};
use crate::storage_backend::NotGarbageCollected;
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

    //TODO TE-437 - get rid of deref call
    fn merkle_ref(&self) -> &dyn KeyValueStoreBackend<MerkleStorage> {
        self.inner.deref() as &dyn KeyValueStoreBackend<MerkleStorage>
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RocksDBBackendStats {
    mem_table_total: u64,
    mem_table_unflushed: u64,
    mem_table_readers_total: u64,
    cache_total: u64,
}

impl NotGarbageCollected for RocksDBBackend {}

impl KeyValueStoreBackend<MerkleStorage> for RocksDBBackend {
    fn is_persistent(&self) -> bool {
        self.merkle_ref().is_persistent()
    }

    fn put(&self, key: &EntryHash, value: &ContextValue) -> Result<(), DBError> {
        self.merkle_ref().put(key, value)
    }

    fn delete(&self, key: &EntryHash) -> Result<(), DBError> {
        self.merkle_ref().delete(key)
    }

    fn merge(&self, key: &EntryHash, value: &ContextValue) -> Result<(), DBError> {
        self.merkle_ref().merge(key, value)
    }

    fn get(&self, key: &EntryHash) -> Result<Option<ContextValue>, DBError> {
        self.merkle_ref().get(key)
    }

    fn contains(&self, key: &EntryHash) -> Result<bool, DBError> {
        self.merkle_ref().contains(key)
    }

    fn retain(&self, predicate: &dyn Fn(&EntryHash) -> bool) -> Result<(), DBError> {
        self.merkle_ref().retain(predicate)
    }

    fn write_batch(&self, batch: Vec<(EntryHash, ContextValue)>) -> Result<(), DBError> {
        self.merkle_ref().write_batch(batch)
    }

    fn total_get_mem_usage(&self) -> Result<usize, DBError> {
        self.merkle_ref().total_get_mem_usage()
    }
}
