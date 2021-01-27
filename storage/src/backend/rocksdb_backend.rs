use crate::storage_backend::{Batch, StorageBackend, StorageBackendError};
use rocksdb::{WriteBatch, WriteOptions, DB};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

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
    fn put(&self, key: Vec<u8>, value: Vec<u8>) -> Result<(), StorageBackendError> {
        let cf = self.inner.cf_handle(&self.column_name).ok_or(
            StorageBackendError::MissingColumnFamily {
                name: &self.column_name,
            },
        )?;

        self.inner
            .put_cf_opt(cf, &key, &value, &Self::default_write_options())
            .map_err(StorageBackendError::from)
    }

    fn merge(&self, key: Vec<u8>, value: Vec<u8>) -> Result<(), StorageBackendError> {
        let cf = self.inner.cf_handle(&self.column_name).ok_or(
            StorageBackendError::MissingColumnFamily {
                name: &self.column_name,
            },
        )?;

        self.inner
            .merge_cf_opt(cf, &key, &value, &Self::default_write_options())
            .map_err(StorageBackendError::from)
    }

    fn delete(&self, key: &Vec<u8>) -> Result<(), StorageBackendError> {
        let cf = self.inner.cf_handle(&self.column_name).ok_or(
            StorageBackendError::MissingColumnFamily {
                name: &self.column_name,
            },
        )?;

        self.inner
            .delete_cf_opt(cf, key, &Self::default_write_options())
            .map_err(StorageBackendError::from)
    }

    fn batch_write(&self, batch: Batch) -> Result<(), StorageBackendError> {
        let mut wb = WriteBatch::default();
        for (k, v) in batch.iter() {
            wb.put(k, v);
        }
        self.inner.write_opt(wb, &Self::default_write_options())?;
        Ok(())
    }

    fn get(&self, key: &Vec<u8>) -> Result<Option<Vec<u8>>, StorageBackendError> {
        let cf = self.inner.cf_handle(&self.column_name).ok_or(
            StorageBackendError::MissingColumnFamily {
                name: &self.column_name,
            },
        )?;

        let v = self
            .inner
            .get_cf(cf, &key)
            .map_err(StorageBackendError::from)?
            .map(|value| value);
        Ok(v)
    }
}
