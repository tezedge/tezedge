use std::collections::{HashMap};
use std::sync::{Arc, RwLock};
use crate::storage_backend::{StorageBackend, StorageBackendStats, StorageBackendError};
use crate::merkle_storage::{EntryHash, ContextValue};

pub struct InMemoryBackend {
    inner: Arc<RwLock<HashMap<EntryHash, ContextValue>>>
}

impl InMemoryBackend {
    pub fn new(db : Arc<RwLock<HashMap<EntryHash, ContextValue>>>) -> Self {
        InMemoryBackend {
            inner: db
        }
    }
}

impl StorageBackend for InMemoryBackend {
    fn is_persisted(&self) -> bool { false }

    fn put(&mut self, key: EntryHash, value: ContextValue) -> Result<bool, StorageBackendError> {
        let mut w = self.inner.write()
            .map(|w| w).map_err(|e| StorageBackendError::GuardPoison {error : format!("{}", e)}
        )?;

        Ok(w.insert(key, value).is_none())
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
            None => {
                Err(StorageBackendError::BackendError)
            }
            Some(v) => {
                Ok(Some(v.clone()))
            }
        }
    }

    fn contains(&self, key: &EntryHash) -> Result<bool, StorageBackendError> {
        let db = self.inner.clone();
        let mut r = db.read()
            .map_err(|e| StorageBackendError::GuardPoison {error : format!("{}", e)}
        )?;
        Ok(r.contains_key(key))
    }

    fn mark_reused(&mut self, key: EntryHash) { }
    fn start_new_cycle(&mut self) { }
    fn wait_for_gc_finish(&self) { }
    fn get_stats(&self) -> Vec<StorageBackendStats> {
      unimplemented!()
    }
}
