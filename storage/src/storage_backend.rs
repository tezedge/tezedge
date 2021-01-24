use std::collections::HashMap;

#[derive(Debug, Fail)]
pub enum StorageBackendError {
    #[fail(display = "RocksDB error: {}", error)]
    RocksDBError { error: rocksdb::Error },
    #[fail(display = "Column family {} is missing", name)]
    MissingColumnFamily { name: &'static str },
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
    fn get<K: AsRef<[u8]>>(&self, key: K) -> Result<Option<Vec<u8>>, StorageBackendError>;
    fn stats(&self) -> Result<Self::StorageStats, StorageBackendError>;
}
