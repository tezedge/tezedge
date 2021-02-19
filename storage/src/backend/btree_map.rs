// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::{btree_map::Entry, BTreeMap};

use crate::merkle_storage::{ContextValue, EntryHash};
use crate::persistent::database::RocksDBStats;
use crate::storage_backend::{StorageBackend as KVStoreTrait, StorageBackendError as KVStoreError};

/// In Memory Key Value Store implemented with [BTreeMap](std::collections::BTreeMap)
#[derive(Debug)]
pub struct KVStore<K: Ord, V> {
    kv_map: BTreeMap<K, V>,
}

impl<K: Ord, V> Default for KVStore<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Ord, V> KVStore<K, V> {
    pub fn new() -> Self {
        Self {
            kv_map: BTreeMap::new(),
        }
    }
}

impl KVStoreTrait for KVStore<EntryHash, ContextValue> {
    fn is_persisted(&self) -> bool {
        false
    }

    /// put kv in map if key doesn't exist. If it does then return false.
    fn put(&mut self, key: &EntryHash, value: ContextValue) -> Result<bool, KVStoreError> {
        match self.kv_map.entry(key.clone()) {
            Entry::Vacant(entry) => {
                entry.insert(value);
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn put_batch(&mut self, batch: Vec<(EntryHash, ContextValue)>) -> Result<(), KVStoreError> {
        for (k, v) in batch.into_iter() {
            self.put(&k, v)?;
        }
        Ok(())
    }

    fn merge(&mut self, key: &EntryHash, value: ContextValue) -> Result<(), KVStoreError> {
        self.kv_map.insert(key.clone(), value);
        Ok(())
    }

    fn delete(&mut self, key: &EntryHash) -> Result<Option<ContextValue>, KVStoreError> {
        Ok(self.kv_map.remove(key))
    }

    fn get(&self, key: &EntryHash) -> Result<Option<ContextValue>, KVStoreError> {
        Ok(self.kv_map.get(key).cloned())
    }

    fn contains(&self, key: &EntryHash) -> Result<bool, KVStoreError> {
        Ok(self.kv_map.contains_key(key))
    }

    fn get_mem_use_stats(&self) -> Result<RocksDBStats, KVStoreError> {
        Ok(RocksDBStats {
            mem_table_total: 0,
            mem_table_unflushed: 0,
            mem_table_readers_total: 0,
            cache_total: 0,
        })
    }
}

pub type BTreeMapBackend = KVStore<EntryHash, ContextValue>;
