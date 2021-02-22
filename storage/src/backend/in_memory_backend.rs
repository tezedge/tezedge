// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::{HashMap, hash_map::Entry, HashSet};
use std::sync::{Arc, RwLock};

use crate::merkle_storage::{ContextValue, EntryHash};
use crate::persistent::database::RocksDBStats;
use crate::storage_backend::{StorageBackend, StorageBackendError, StorageBackendStats};

#[derive(Default)]
pub struct InMemoryBackend {
    inner: Arc<RwLock<HashMap<EntryHash, ContextValue>>>,
    stats: StorageBackendStats,
}

impl InMemoryBackend {
    pub fn new() -> Self {
        InMemoryBackend {
            inner: Arc::new(RwLock::new(HashMap::new())),
            stats: Default::default(),
        }
    }
}

impl StorageBackend for InMemoryBackend {
    fn is_persisted(&self) -> bool {
        false
    }

    fn put(&mut self, key: &EntryHash, value: ContextValue) -> Result<bool, StorageBackendError> {
        let measurement = StorageBackendStats::from((key, &value));
        let mut w = self
            .inner
            .write()
            .map_err(|e| StorageBackendError::GuardPoison {
                error: format!("{}", e),
            })?;

        if w.contains_key(key){
            Ok(false)
        }else{
            w.insert(*key,value);
            self.stats += measurement;
            Ok(true)
        }
    }

    fn merge(&mut self, key: &EntryHash, value: ContextValue) -> Result<(), StorageBackendError> {
        let measurement = StorageBackendStats::from((key, &value));
        let mut w = self
            .inner
            .write()
            .map_err(|e| StorageBackendError::GuardPoison {
                error: format!("{}", e),
            })?;


        if let Some(prev) = w.insert(*key, value){
            self.stats -= StorageBackendStats::from((key, &prev));
        };
        self.stats += measurement;
        Ok(())
    }

    fn delete(&mut self, key: &EntryHash) -> Result<Option<ContextValue>, StorageBackendError> {
        let mut w = self
            .inner
            .write()
            .map_err(|e| StorageBackendError::GuardPoison {
                error: format!("{}", e),
            })?;

        let removed_key = w.remove(key);

        if let Some(v) =  &removed_key{
            self.stats -= StorageBackendStats::from((key, v));
        }

        Ok(removed_key)
    }

    fn get(&self, key: &EntryHash) -> Result<Option<ContextValue>, StorageBackendError> {
        let db = self.inner.clone();
        let r = db.read().map_err(|e| StorageBackendError::GuardPoison {
            error: format!("{}", e),
        })?;

        match r.get(key) {
            None => Ok(None),
            Some(v) => Ok(Some(v.clone())),
        }
    }

    fn contains(&self, key: &EntryHash) -> Result<bool, StorageBackendError> {
        let db = self.inner.clone();
        let r = db.read().map_err(|e| StorageBackendError::GuardPoison {
            error: format!("{}", e),
        })?;
        Ok(r.contains_key(key))
    }

    fn retain(&mut self, pred: HashSet<EntryHash>) -> Result<(), StorageBackendError> {
        let garbage_keys: Vec<_> = self
            .inner
            .read()
            .unwrap()
            .iter()
            .filter_map(|(k, _)| {
                if !pred.contains(k) {
                    Some(*k)
                } else {
                    None
                }
            })
            .collect();

        for k in garbage_keys {
            self.delete(&k)?;
        }
        Ok(())
    }

    fn total_get_mem_usage(&self) -> Result<usize,StorageBackendError>{
        Ok(self.stats.total_as_bytes())
    }
}
