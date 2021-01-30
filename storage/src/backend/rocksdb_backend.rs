use rocksdb::{DB, WriteOptions};
use std::sync::Arc;
use serde::{Serialize, Deserialize};
use crate::storage_backend::{StorageBackend, StorageBackendStats, StorageBackendError};
use crate::merkle_storage::{EntryHash, ContextValue};
use std::collections::HashSet;

pub struct RocksDBBackend {
    column_name: &'static str,
    inner: Arc<DB>,
}

impl RocksDBBackend {
    pub fn new(db: Arc<DB>, column_name: &'static str) -> Self {
        RocksDBBackend {
            inner: db,
            column_name,
        }
    }
}

impl RocksDBBackend {
    fn default_write_options() -> WriteOptions {
        let mut opts = WriteOptions::default();
        opts.set_sync(false);
        opts
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RocksDBBackendStats {
    mem_table_total: u64,
    mem_table_unflushed: u64,
    mem_table_readers_total: u64,
    cache_total: u64,
}


impl StorageBackend for RocksDBBackend {
    fn is_persisted(&self) -> bool { true }

    /// Warning: always returns true.
    fn put(&mut self, key: EntryHash, value: ContextValue) -> Result<bool, StorageBackendError> {
        let cf = self.inner
            .cf_handle(&self.column_name)
            .ok_or(StorageBackendError::MissingColumnFamily { name: &self.column_name })?;

        self.inner.put_cf_opt(cf, &key, &value, &Self::default_write_options())
            .map_err(StorageBackendError::from)
            .map(|_| true)
    }

    fn merge(&mut self, key: EntryHash, value: ContextValue) -> Result<(), StorageBackendError> {
        let cf = self.inner
            .cf_handle(&self.column_name)
            .ok_or(StorageBackendError::MissingColumnFamily { name: &self.column_name })?;

        self.inner.merge_cf_opt(cf, &key, &value, &Self::default_write_options())
            .map_err(StorageBackendError::from)
    }

    /// Warning: always returns None.
    fn delete(&mut self, key: &EntryHash) -> Result<Option<ContextValue>, StorageBackendError> {
        let cf = self.inner
            .cf_handle(&self.column_name)
            .ok_or(StorageBackendError::MissingColumnFamily { name: &self.column_name })?;

        self.inner.delete_cf_opt(cf, key, &Self::default_write_options())
            .map_err(StorageBackendError::from)?;
        Ok(None)
    }

    fn get(&self, key: &EntryHash) -> Result<Option<ContextValue>, StorageBackendError> {
        let cf = self.inner
            .cf_handle(&self.column_name)
            .ok_or(StorageBackendError::MissingColumnFamily { name: &self.column_name })?;

        let v = self.inner.get_cf(cf, &key)
            .map_err(StorageBackendError::from)?
            .map(|value| value);
        Ok(v)
    }

    fn contains(&self, key: &EntryHash) -> Result<bool, StorageBackendError> {
        self.get(key).map(|v| v.is_some())
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
