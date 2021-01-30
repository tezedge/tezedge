use sled::IVec;

use crate::storage_backend::{StorageBackend, StorageBackendStats, StorageBackendError};
use crate::merkle_storage::{EntryHash, ContextValue};
use std::collections::HashSet;

pub struct SledBackend {
    inner: sled::Tree
}

impl SledBackend {
    pub fn new(db : sled::Tree) -> Self{
        SledBackend {
            inner: db
        }
    }
}

impl StorageBackend for SledBackend {
    fn is_persisted(&self) -> bool { true }

    fn put(&mut self, key: EntryHash, value: ContextValue) -> Result<bool, StorageBackendError> {
        Ok(self.inner.insert(key, value).map(|v| v.is_none())?)
    }

    fn merge(&mut self, key: EntryHash, value: ContextValue) -> Result<(), StorageBackendError> {
        self.inner.insert(key, value)?;
        Ok(())
    }

    fn delete(&mut self, key: &EntryHash) -> Result<Option<ContextValue>, StorageBackendError> {
        Ok(self.inner.remove(key)?.map(|v| v.to_vec()))
    }

    fn get(&self, key: &EntryHash) -> Result<Option<ContextValue>, StorageBackendError> {
        let r = self.inner.get(key)?;

        match r {
            None => {
                Err(StorageBackendError::BackendError)
            }
            Some(v) => {
                Ok(Some(v.to_vec()))
            }
        }
    }

    fn contains(&self, key: &EntryHash) -> Result<bool, StorageBackendError> {
        Ok(self.inner.contains_key(key)?)
    }

    fn retain(&mut self, pred: HashSet<EntryHash>) -> Result<(), StorageBackendError> {
        unimplemented!()
    }

    fn mark_reused(&mut self, key: EntryHash) { }
    fn start_new_cycle(&mut self, _last_commit_hash: Option<EntryHash>) { }
    fn wait_for_gc_finish(&self) { }
    fn get_stats(&self) -> Vec<StorageBackendStats> {
      unimplemented!()
    }
}
