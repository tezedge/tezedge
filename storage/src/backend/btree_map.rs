use std::collections::{BTreeMap, btree_map::Entry};
use rayon::prelude::*;

use crate::storage_backend::{
    StorageBackend as KVStoreTrait,
    StorageBackendStats as KVStoreStats,
    StorageBackendError as KVStoreError,
};
use crate::merkle_storage::{EntryHash, ContextValue};


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
    fn new() -> Self {
        Self { kv_map: BTreeMap::new() }
    }
}

impl KVStoreTrait for KVStore<EntryHash, ContextValue> {
    fn is_persisted(&self) -> bool { false }

    /// put kv in map if key doesn't exist. If it does then return false.
    fn put(&mut self, key: EntryHash, value: ContextValue) -> Result<bool, KVStoreError> {
        match self.kv_map.entry(key) {
            Entry::Vacant(entry) => {
                entry.insert(value);
                Ok(true)
            },
            // _ => Err(KVStoreError::EntryOccupied),
            _ => Ok(false)
        }
    }

    fn merge(&mut self, key: EntryHash, value: ContextValue) -> Result<(), KVStoreError> {
        self.kv_map.insert(key, value);
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

    fn retain(&mut self, pred: Vec<EntryHash>) -> Result<(), KVStoreError> {
        let garbage_keys: Vec<_> = self.kv_map.par_iter().filter_map(|(k, v)| {
            if !pred.contains(&k) {
                Some(k.clone())
            } else {
                None
            }
        }).collect();

        for k in garbage_keys {
            self.delete(&k)?;
        }
        Ok(())
    }

    fn mark_reused(&mut self, key: EntryHash) { }
    fn start_new_cycle(&mut self, _last_commit_hash: Option<EntryHash>) { }
    fn wait_for_gc_finish(&self) { }
    fn get_stats(&self) -> Vec<KVStoreStats> {
      unimplemented!()
    }
}

pub type BTreeMapBackend = KVStore<EntryHash, ContextValue>;
