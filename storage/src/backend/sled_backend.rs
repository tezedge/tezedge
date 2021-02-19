use crate::merkle_storage::{ContextValue, EntryHash};
use crate::persistent::database::RocksDBStats;
use crate::storage_backend::{StorageBackend, StorageBackendError, StorageBackendStats};

pub struct SledBackend {
    inner: sled::Tree,
}

impl SledBackend {
    pub fn new(db: sled::Tree) -> Self {
        SledBackend { inner: db }
    }
}

impl StorageBackend for SledBackend {
    fn is_persisted(&self) -> bool {
        true
    }

    fn put(&mut self, key: EntryHash, value: ContextValue) -> Result<bool, StorageBackendError> {
        Ok(self
            .inner
            .insert(&key.as_ref()[..], value)
            .map(|v| v.is_none())?)
    }

    fn merge(&mut self, key: EntryHash, value: ContextValue) -> Result<(), StorageBackendError> {
        self.inner.insert(&key.as_ref()[..], value)?;
        Ok(())
    }

    fn delete(&mut self, key: &EntryHash) -> Result<Option<ContextValue>, StorageBackendError> {
        Ok(self.inner.remove(&key.as_ref()[..])?.map(|v| v.to_vec()))
    }

    fn get(&self, key: &EntryHash) -> Result<Option<ContextValue>, StorageBackendError> {
        let r = self.inner.get(&key.as_ref()[..])?;

        match r {
            None => Err(StorageBackendError::BackendError),
            Some(v) => Ok(Some(v.to_vec())),
        }
    }

    fn contains(&self, key: &EntryHash) -> Result<bool, StorageBackendError> {
        Ok(self.inner.contains_key(&key.as_ref()[..])?)
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
