use rocksdb::{DB, WriteOptions, WriteBatch};
use std::sync::Arc;
use crate::storage_backend::{StorageBackend, StorageBackendError, Batch};

pub struct RocksDBBackend {
    column_name: String,
    inner: Arc<DB>,
}

impl RocksDBBackend {
    pub fn new(db: Arc<DB>, column_name: String) -> Self {
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
    type StorageStats = RocksDBBackendStats;

    fn put(&self, key: Vec<u8>, value: Vec<u8>) -> Result<(), StorageBackendError> {
        let cf = self.inner
            .cf_handle(&self.column_name)
            .ok_or(StorageBackendError::MissingColumnFamily { name: &self.column_name })?;

        self.inner.put_cf_opt(cf, &key, &value, &RocksDBBackend::default_write_options())
            .map_err(StorageBackendError::from)
    }

    fn merge(&self, key: Vec<u8>, value: Vec<u8>) -> Result<(), StorageBackendError> {
        let cf = self.inner
            .cf_handle(&self.column_name)
            .ok_or(StorageBackendError::MissingColumnFamily { name: &self.column_name })?;

        self.inner.merge_cf_opt(cf, &key, &value, &default_write_options())
            .map_err(StorageBackendError::from)
    }

    fn delete(&self, key: &Vec<u8>) -> Result<(), StorageBackendError> {
        let cf = self.inner
            .cf_handle(&self.column_name)
            .ok_or(StorageBackendError::MissingColumnFamily { name: &self.column_name })?;

        self.inner.delete_cf_opt(cf, key, &RocksDBBackend::default_write_options())
            .map_err(StorageBackendError::from)
    }

    fn batch_write(&self, batch: Batch) -> Result<(), StorageBackendError> {
        let mut wb = WriteBatch::default();
        batch.iterator(|(k, v)| {
            wb.put(k, v);
        });
        self.inner.write_opt(wb, &RocksDBBackend::default_write_options())?;
        Ok(())
    }

    fn get<K: AsRef<[u8]>>(&self, key: K) -> Result<Option<Vec<u8>>, StorageBackendError> {
        let cf = self.inner
            .cf_handle(&self.column_name)
            .ok_or(StorageBackendError::MissingColumnFamily { name: &self.column_name })?;

        let v = self.inner.get_cf(cf, &key)
            .map_err(StorageBackendError::from)?
            .map(|value| value);
        Ok(v)
    }

    fn stats(&self) -> Result<Self::StorageStats, StorageBackendError> {
        let memory_usage_stats = rocksdb::perf::get_memory_usage_stats(Some(&[&self.inner]), None)?;

        Ok(RocksDBBackendStats {
            mem_table_total: memory_usage_stats.mem_table_total,
            mem_table_unflushed: memory_usage_stats.mem_table_unflushed,
            mem_table_readers_total: memory_usage_stats.mem_table_readers_total,
            cache_total: memory_usage_stats.cache_total,
        })
    }
}