// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

use crate::merkle_storage::{ContextValue, EntryHash};
use crate::persistent::database::{DBError, KeyValueStoreBackend};
use crate::storage_backend::{GarbageCollector, StorageBackendError, StorageBackendStats};
use crate::MerkleStorage;
use std::ops::{AddAssign, DerefMut, SubAssign};

#[derive(Default)]
pub struct InMemoryBackend {
    inner: Arc<RwLock<HashMap<EntryHash, ContextValue>>>,
    stats: Mutex<StorageBackendStats>,
}

impl InMemoryBackend {
    pub fn new() -> Self {
        InMemoryBackend {
            inner: Arc::new(RwLock::new(HashMap::new())),
            stats: Default::default(),
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
        let measurement = StorageBackendStats::from((key, value));
        let mut w = self.inner.write().map_err(|e| DBError::GuardPoison {
            error: format!("{}", e),
        })?;

        if let Some(val) = w.get(key) {
            self.stats
                .lock()
                .unwrap()
                .deref_mut()
                .sub_assign(StorageBackendStats::from((key, val)));
        }

        w.insert(*key, value.clone());
        self.stats
            .lock()
            .unwrap()
            .deref_mut()
            .add_assign(measurement);
        Ok(())
    }

    fn delete(&self, key: &EntryHash) -> Result<(), DBError> {
        let mut w = self.inner.write().map_err(|e| DBError::GuardPoison {
            error: format!("{}", e),
        })?;

        let removed_key = w.remove(key);

        if let Some(v) = &removed_key {
            self.stats
                .lock()
                .unwrap()
                .deref_mut()
                .sub_assign(StorageBackendStats::from((key, v)));
        }

        Ok(())
    }

    fn merge(&self, key: &EntryHash, value: &ContextValue) -> Result<(), DBError> {
        let measurement = StorageBackendStats::from((key, value));
        let mut w = self.inner.write().map_err(|e| DBError::GuardPoison {
            error: format!("{}", e),
        })?;

        if let Some(prev) = w.insert(*key, value.clone()) {
            self.stats
                .lock()
                .unwrap()
                .deref_mut()
                .sub_assign(StorageBackendStats::from((key, &prev)));
        };
        self.stats
            .lock()
            .unwrap()
            .deref_mut()
            .add_assign(measurement);
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
        Ok(self.stats.lock().unwrap().total_as_bytes())
    }

    fn is_persistent(&self) -> bool {
        false
    }
}
