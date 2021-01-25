use failure::{Fail, Error};

use crate::persistent::SchemaError;
use crate::persistent::SledError;

#[derive(Debug, Fail)]
pub enum KVStoreError {
    #[fail(display = "Entry with key already exists")]
    EntryOccupied,
    #[fail(display = "Schema error: {}", error)]
    SchemaError {
        error: SchemaError,
    },
    #[fail(display = "Sled error: {}", error)]
    SledError { error: SledError },
}

impl From<SchemaError> for KVStoreError {
    fn from(error: SchemaError) -> Self {
        KVStoreError::SchemaError { error }
    }
}

impl<E: Into<SledError>> From<E> for KVStoreError {
    fn from(error: E) -> Self {
        KVStoreError::SledError { error: error.into() }
    }
}

pub trait WriteBatch {
    type Key;
    type Value;

    fn put(&mut self, key: Self::Key, value: Self::Value);
    fn merge(&mut self, key: Self::Key, value: Self::Value);
    fn delete(&mut self, key: Self::Key);
}

pub trait KVStore {
    type Error;
    type Key;
    type Value;

    fn is_persisted(&self) -> bool;

    /// put kv in map if key doesn't exist. If it does then return false.
    fn put(&mut self, key: Self::Key, value: Self::Value) -> Result<bool, Self::Error>;

    fn merge(&mut self, key: Self::Key, value: Self::Value) -> Result<Option<Self::Value>, Self::Error>;

    fn delete(&mut self, key: &Self::Key) -> Result<Option<Self::Value>, Self::Error>;

    fn get(&self, key: &Self::Key) -> Result<Option<Self::Value>, Self::Error>;

    fn contains(&self, key: &Self::Key) -> Result<bool, Self::Error>;

    fn len(&self) -> Result<usize, Self::Error>;
}

pub trait ApplyBatch<WB, E>
where WB: WriteBatch
{
    /// atomically apply batch
    fn apply_batch(&mut self, batch: WB) -> Result<(), E>;
}


#[derive(Debug)]
pub enum BasicWriteBatchOp<K, V> {
    Put { key: K, value: V },
    Merge { key: K, value: V },
    Delete { key: K },
}

#[derive(Debug)]
pub struct BasicWriteBatch<K, V> {
    ops: Vec<BasicWriteBatchOp<K, V>>
}

impl<K, V> BasicWriteBatch<K, V> {
    pub fn new() -> Self {
        Self { ops: Vec::new() }
    }

    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, BasicWriteBatchOp<K, V>> {
        self.into_iter()
    }
}

impl<K, V> Default for BasicWriteBatch<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> WriteBatch for BasicWriteBatch<K, V> {
    type Key = K;
    type Value = V;

    fn put(&mut self, key: Self::Key, value: Self::Value) {
        self.ops.push(BasicWriteBatchOp::Put { key, value });
    }

    fn merge(&mut self, key: Self::Key, value: Self::Value) {
        self.ops.push(BasicWriteBatchOp::Merge { key, value });
    }

    fn delete(&mut self, key: Self::Key) {
        self.ops.push(BasicWriteBatchOp::Delete { key });
    }
}

impl<K, V> IntoIterator for BasicWriteBatch<K, V> {
    type Item = BasicWriteBatchOp<K, V>;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.ops.into_iter()
    }
}

impl<'a, K, V> IntoIterator for &'a BasicWriteBatch<K, V> {
    type Item = &'a BasicWriteBatchOp<K, V>;
    type IntoIter = std::slice::Iter<'a, BasicWriteBatchOp<K, V>>;

    fn into_iter(self) -> Self::IntoIter {
        self.ops.iter()
    }
}
