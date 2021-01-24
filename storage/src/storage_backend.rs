use std::collections::HashMap;
use crate::persistent::codec::{Decoder, Encoder, SchemaError};
use crate::persistent::schema::KeyValueSchema;
use rocksdb::{DB, WriteOptions, WriteBatch};
use serde::Deserialize;
use std::sync::Arc;
use std::collections::hash_map::Iter;
use serde_json::Value;

#[derive(Debug, Fail)]
pub enum StorageBackendError {
    #[fail(display = "Schema error: {}", error)]
    SchemaError { error: SchemaError },
    #[fail(display = "RocksDB error: {}", error)]
    RocksDBError { error: rocksdb::Error },
    #[fail(display = "Column family {} is missing", name)]
    MissingColumnFamily { name: &'static str },
}

impl From<SchemaError> for StorageBackendError {
    fn from(error: SchemaError) -> Self {
        StorageBackendError::SchemaError { error }
    }
}

impl From<rocksdb::Error> for StorageBackendError {
    fn from(error: rocksdb::Error) -> Self {
        StorageBackendError::RocksDBError { error }
    }
}

impl slog::Value for StorageBackendError {
    fn serialize(
        &self,
        _record: &slog::Record,
        key: slog::Key,
        serializer: &mut dyn slog::Serializer,
    ) -> slog::Result {
        serializer.emit_arguments(key, &format_args!("{}", self))
    }
}

pub struct Batch {
    inner: HashMap<Vec<u8>, Vec<u8>>
}

impl Batch {
    pub fn update(&mut self, key: Vec<u8>, value: Vec<u8>) {
        self.inner.insert(key, value);
    }

    pub fn delete(&mut self, key: &Vec<u8>) {
        self.inner.remove(key);
    }

    pub fn iterator(&self, f: fn((Vec<u8>, Vec<u8>))) {
        for (k, v) in self.inner {
            f((k, v))
        }
    }
}


pub trait StorageBackend {
    type StorageStats;

    fn put(&self, key: Vec<u8>, value: Vec<u8>) -> Result<(), StorageBackendError>;
    fn merge(&self, key: Vec<u8>, value: Vec<u8>) -> Result<(), StorageBackendError>;
    fn delete(&self, key: &Vec<u8>) -> Result<(), StorageBackendError>;
    fn batch_write(&self, batch: Batch) -> Result<(), StorageBackendError>;
    fn get(&self, key: &Vec<u8>) -> Result<Option<Vec<u8>>, StorageBackendError>;
    fn stats(&self) -> Self::StorageStats;
}


pub struct RocksDBBackend {
    stats : RocksDBBackendStats,
    column_name: String,
    inner: Arc<DB>,
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

    fn get(&self, key: &Vec<u8>) -> Result<Option<Vec<u8>>, StorageBackendError> {
        let cf = self.inner
            .cf_handle(&self.column_name)
            .ok_or(StorageBackendError::MissingColumnFamily { name : &self.column_name })?;

        let v = self.inner.get_cf(cf, &key)
            .map_err(StorageBackendError::from)?
            .map(|value| value);
        Ok(v)
    }

    fn stats(&self) -> Self::StorageStats {
        return self.stats.clone()
    }
}