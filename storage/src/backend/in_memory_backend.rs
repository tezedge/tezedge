// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::{Arc, RwLock};

use crate::merkle_storage::{ContextValue, EntryHash};
use crate::persistent::database::{DBError, KeyValueStoreBackend};
use crate::storage_backend::{GarbageCollector, StorageBackendError};
use crate::MerkleStorage;
use std::collections::HashMap;
use crate::storage_backend::StorageBackendStats;

#[derive(Default)]
pub struct HashMapWithStats{
    inner: HashMap<EntryHash,ContextValue>,
    stats: StorageBackendStats,
}

impl HashMapWithStats
{
    pub fn insert(&mut self, key: EntryHash, value: ContextValue) -> Option<ContextValue>{
        let stats = StorageBackendStats::from((&key, &value));
        match self.inner.insert(key,value){
            Some(prev) => {
                self.stats -= StorageBackendStats::from((&key, &prev));
                self.stats += stats;
                Some(prev)
            }
            None => {
                self.stats += stats;
                None
            }
        }
    }

    pub fn remove(&mut self, key: &EntryHash) -> Option<ContextValue>{
        match self.inner.remove(key){
            Some(prev) => {
                self.stats -= StorageBackendStats::from((key, &prev));
                Some(prev)
            },
            None => None
        }
    }

    pub fn get(&self, key: &EntryHash) -> Option<&ContextValue>{
        self.inner.get(key)
    }

    pub fn contains_key(&self, key: &EntryHash) -> bool{
        self.inner.contains_key(key)
    }

    pub fn iter(&self) -> std::collections::hash_map::Iter<EntryHash,ContextValue>{
        self.inner.iter()
    }

    pub fn get_memory_usage(&self) -> StorageBackendStats{
        self.stats
    }
}

#[derive(Default)]
pub struct InMemoryBackend {
    inner: Arc<RwLock<HashMapWithStats>>
}

impl InMemoryBackend {
    pub fn new() -> Self {
        InMemoryBackend {
            inner: Arc::new(RwLock::new(HashMapWithStats::default())),
        }
    }
}

impl GarbageCollector for InMemoryBackend {
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

impl KeyValueStoreBackend<MerkleStorage> for InMemoryBackend {
    fn retain(&self, predicate: &dyn Fn(&EntryHash) -> bool) -> Result<(), DBError> {
        let garbage_keys: Vec<_> = self
            .inner
            .read()
            .unwrap()
            .iter()
            .filter_map(|(k, _)| if !predicate(k) { Some(*k) } else { None })
            .collect();

        for k in garbage_keys {
            self.delete(&k)?;
        }
        Ok(())
    }

    fn put(&self, key: &EntryHash, value: &ContextValue) -> Result<(), DBError> {
        let mut w = self.inner.write().map_err(|e| DBError::GuardPoison {
            error: format!("{}", e),
        })?;
        w.insert(*key, value.clone());
        Ok(())
    }

    fn delete(&self, key: &EntryHash) -> Result<(), DBError> {
        let mut w = self.inner.write().map_err(|e| DBError::GuardPoison {
            error: format!("{}", e),
        })?;
        w.remove(key);
        Ok(())
    }

    fn merge(&self, key: &EntryHash, value: &ContextValue) -> Result<(), DBError> {
        let mut w = self.inner.write().map_err(|e| DBError::GuardPoison {
            error: format!("{}", e),
        })?;

        w.insert(*key, value.clone());
        Ok(())
    }

    fn get(&self, key: &EntryHash) -> Result<Option<ContextValue>, DBError> {
        let db = self.inner.clone();
        let r = db.read().map_err(|e| DBError::GuardPoison {
            error: format!("{}", e),
        })?;

        match r.get(key) {
            None => Ok(None),
            Some(v) => Ok(Some(v.clone())),
        }
    }

    fn contains(&self, key: &EntryHash) -> Result<bool, DBError> {
        let db = self.inner.clone();
        let r = db.read().map_err(|e| DBError::GuardPoison {
            error: format!("{}", e),
        })?;
        Ok(r.contains_key(key))
    }

    fn write_batch(&self, batch: Vec<(EntryHash, ContextValue)>) -> Result<(), DBError> {
        for (k, v) in batch {
            self.merge(&k, &v)?;
        }
        Ok(())
    }

    fn total_get_mem_usage(&self) -> Result<usize, DBError> {
        let r = self.inner.read().map_err(|e| DBError::GuardPoison {
            error: format!("{}", e),
        })?;
        Ok(r.get_memory_usage().total_as_bytes())
    }

    fn is_persistent(&self) -> bool {
        false
    }
}
