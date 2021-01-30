use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use rayon::prelude::*;

use crate::storage_backend::{StorageBackend, StorageBackendStats, StorageBackendError};
use crate::merkle_storage::{EntryHash, ContextValue};

#[derive(Default)]
pub struct InMemoryBackend {
    inner: Arc<RwLock<HashMap<EntryHash, ContextValue>>>,
    stats: StorageBackendStats,
}

impl InMemoryBackend {
    pub fn new(db : Arc<RwLock<HashMap<EntryHash, ContextValue>>>) -> Self {
        InMemoryBackend {
            inner: db,
            stats: Default::default(),
        }
    }
}

impl StorageBackend for InMemoryBackend {
    fn is_persisted(&self) -> bool { false }

    fn put(&mut self, key: EntryHash, value: ContextValue) -> Result<bool, StorageBackendError> {
        let measurement = StorageBackendStats::from((&key, &value));
        let mut w = self.inner.write()
            .map(|w| w).map_err(|e| StorageBackendError::GuardPoison {error : format!("{}", e)})?;

        let was_added = w.insert(key, value).is_none();

        if was_added {
            self.stats += measurement;
        }

        Ok(was_added)
    }

    fn merge(&mut self, key: EntryHash, value: ContextValue) -> Result<(), StorageBackendError> {
        let mut w = self.inner.write()
            .map(|w| w).map_err(|e| StorageBackendError::GuardPoison {error : format!("{}", e)}
        )?;

        w.insert(key,value);
        Ok(())
    }

    fn delete(&mut self, key: &EntryHash) -> Result<Option<ContextValue>, StorageBackendError> {
        let mut w = self.inner.write()
            .map(|w| w).map_err(|e| StorageBackendError::GuardPoison {error : format!("{}", e)}
        )?;

        Ok(w.remove(key))
    }

    fn get(&self, key: &EntryHash) -> Result<Option<ContextValue>, StorageBackendError> {
        let db = self.inner.clone();
        let mut r = db.read()
            .map_err(|e| StorageBackendError::GuardPoison {error : format!("{}", e)}
        )?;

         match r.get(key) {
            None => Ok(None),
            Some(v) => Ok(Some(v.clone())),
        }
    }

    fn contains(&self, key: &EntryHash) -> Result<bool, StorageBackendError> {
        let db = self.inner.clone();
        let mut r = db.read()
            .map_err(|e| StorageBackendError::GuardPoison {error : format!("{}", e)}
        )?;
        Ok(r.contains_key(key))
    }

    fn retain(&mut self, pred: HashSet<EntryHash>) -> Result<(), StorageBackendError> {
        let garbage_keys: Vec<_> = self.inner.read().unwrap()
            .par_iter().filter_map(|(k, v)| {
                if !pred.contains(&k[..]) {
                    Some(k.clone())
                } else {
                    None
                }
            })
            .collect();

        let mut writer = self.inner.write().unwrap();
        for k in garbage_keys {
            match writer.remove(&k) {
                Some(v) => self.stats -= StorageBackendStats::from((&k, &v)),
                None => (),
            }
        }
        Ok(())
    }

    fn mark_reused(&mut self, key: EntryHash) { }
    fn start_new_cycle(&mut self, _last_commit_hash: Option<EntryHash>) { }
    fn wait_for_gc_finish(&self) { }
    fn get_stats(&self) -> Vec<StorageBackendStats> {
        vec![self.stats]
    }
}
