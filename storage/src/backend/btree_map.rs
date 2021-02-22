// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::{btree_map::Entry, BTreeMap};

use crate::merkle_storage::{ContextValue, EntryHash};
use crate::storage_backend::{StorageBackend , StorageBackendError, StorageBackendStats};

/// In Memory Key Value Store implemented with [BTreeMap](std::collections::BTreeMap)
#[derive(Debug)]
pub struct KVStore<K: Ord, V> {
    kv_map: BTreeMap<K, V>,
    stats: StorageBackendStats,
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
            stats: Default::default(),
        }
    }
}

impl StorageBackend for KVStore<EntryHash, ContextValue> {
    fn is_persisted(&self) -> bool {
        false
    }

    fn total_get_mem_usage(&self) -> Result<usize,StorageBackendError>{
        Ok(self.stats.total_as_bytes())
    }

    /// put kv in map if key doesn't exist. If it does then return false.
    fn put(&mut self, key: &EntryHash, value: ContextValue) -> Result<bool, StorageBackendError> {
        match self.kv_map.entry(*key) {
            Entry::Vacant(entry) => {
                self.stats += StorageBackendStats::from((key, &value));
                entry.insert(value);
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn merge(&mut self, key: &EntryHash, value: ContextValue) -> Result<(), StorageBackendError> {
        self.stats += StorageBackendStats::from((key, &value));
        if let Some(prev) = self.kv_map.insert(*key, value){
            self.stats -= StorageBackendStats::from((key, &prev));
        };
        Ok(())
    }

    fn delete(&mut self, key: &EntryHash) -> Result<Option<ContextValue>, StorageBackendError> {
        let removed_key = self.kv_map.remove(key);

        if let Some(v) =  &removed_key{
            self.stats -= StorageBackendStats::from((key, v));
        }

        Ok(removed_key)
    }

    fn get(&self, key: &EntryHash) -> Result<Option<ContextValue>, StorageBackendError> {
        Ok(self.kv_map.get(key).cloned())
    }

    fn contains(&self, key: &EntryHash) -> Result<bool, StorageBackendError> {
        Ok(self.kv_map.contains_key(key))
    }
}

pub type BTreeMapBackend = KVStore<EntryHash, ContextValue>;
