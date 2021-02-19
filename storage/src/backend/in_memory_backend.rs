// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;
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

        let was_added = w.insert(key.clone(), value).is_none();

        if was_added {
            self.stats += measurement;
        }

        Ok(was_added)
    }

    fn put_batch(
        &mut self,
        batch: Vec<(EntryHash, ContextValue)>,
    ) -> Result<(), StorageBackendError> {
        for (k, v) in batch.into_iter() {
            self.put(&k, v)?;
        }
        Ok(())
    }

    fn merge(&mut self, key: &EntryHash, value: ContextValue) -> Result<(), StorageBackendError> {
        let mut w = self
            .inner
            .write()
            .map_err(|e| StorageBackendError::GuardPoison {
                error: format!("{}", e),
            })?;

        w.insert(key.clone(), value);
        Ok(())
    }

    fn delete(&mut self, key: &EntryHash) -> Result<Option<ContextValue>, StorageBackendError> {
        let mut w = self
            .inner
            .write()
            .map_err(|e| StorageBackendError::GuardPoison {
                error: format!("{}", e),
            })?;

        Ok(w.remove(key))
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

    fn get_mem_use_stats(&self) -> Result<RocksDBStats, StorageBackendError> {
        Ok(RocksDBStats {
            mem_table_total: 0,
            mem_table_unflushed: 0,
            mem_table_readers_total: 0,
            cache_total: 0,
        })
    }
}
