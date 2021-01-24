use std::collections::HashMap;
use failure::Fail;
use std::collections::hash_map::Iter;

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
impl Default for Batch {
    fn default() -> Self {
        Batch {
            inner: Default::default()
        }
    }
}
impl Batch {
    pub fn put(&mut self, key: Vec<u8>, value: Vec<u8>) {
        self.inner.insert(key, value);
    }

    pub fn delete(&mut self, key: &Vec<u8>) {
        self.inner.remove(key);
    }

    pub fn iter(&self) -> Iter<Vec<u8>,Vec<u8>> {
        self.inner.iter()
    }
}


pub trait StorageBackend {

    fn put(&self, key: Vec<u8>, value: Vec<u8>) -> Result<(), StorageBackendError>;
    fn merge(&self, key: Vec<u8>, value: Vec<u8>) -> Result<(), StorageBackendError>;
    fn delete(&self, key: &Vec<u8>) -> Result<(), StorageBackendError>;
    fn batch_write(&self, batch: Batch) -> Result<(), StorageBackendError>;
    fn get(&self, key: &Vec<u8>) -> Result<Option<Vec<u8>>, StorageBackendError>;
}
