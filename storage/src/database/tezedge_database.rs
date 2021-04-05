use crate::block_storage::{BlockByContextHashIndex, BlockByLevelIndex, BlockPrimaryIndex};
use crate::database::backend::{
    BackendIterator, BackendIteratorMode, TezedgeDatabaseBackendStore,
    TezedgeDatabaseBackendStoreIterator,
};
use crate::database::error::Error;
use crate::database::notus_backend::NotusDBBackend;
use crate::database::sled_backend::SledDBBackend;
use crate::persistent::sequence::Sequences;
use crate::persistent::{Decoder, Encoder, KeyValueSchema, SchemaError};
use crate::{
    BlockMetaStorage, ChainMetaStorage, IteratorMode, OperationsMetaStorage, OperationsStorage,
    PredecessorStorage, SystemStorage,
};
use im::HashMap;
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

pub trait KVStoreKeyValueSchema: KeyValueSchema {
    fn column_name() -> &'static str;
}

/// Custom trait to unify any kv-store schema access
pub trait KVStore<S: KeyValueSchema> {
    /// Insert new key value pair into the database.
    ///
    /// # Arguments
    /// * `key` - Value of key specified by schema
    /// * `value` - Value to be inserted associated with given key, specified by schema
    fn put(&self, key: &S::Key, value: &S::Value) -> Result<(), Error>;

    /// Delete existing value associated with given key from the database.
    ///
    /// # Arguments
    /// * `key` - Value of key specified by schema
    fn delete(&self, key: &S::Key) -> Result<(), Error>;

    /// Insert key value pair into the database, overriding existing value if exists.
    ///
    /// # Arguments
    /// * `key` - Value of key specified by schema
    /// * `value` - Value to be inserted associated with given key, specified by schema
    fn merge(&self, key: &S::Key, value: &S::Value) -> Result<(), Error>;

    /// Read value associated with given key, if exists.
    ///
    /// # Arguments
    /// * `key` - Value of key specified by schema
    fn get(&self, key: &S::Key) -> Result<Option<S::Value>, Error>;

    /// Check, if database contains given key
    ///
    /// # Arguments
    /// * `key` - Key (specified by schema), to be checked for existence
    fn contains(&self, key: &S::Key) -> Result<bool, Error>;

    /// Write batch into DB atomically
    ///
    /// # Arguments
    /// * `batch` - WriteBatch containing all batched writes to be written to DB
    fn write_batch(&self, batch: Vec<(S::Key, S::Value)>) -> Result<(), Error>;
}

pub trait KVStoreWithSchemaIterator<S: KeyValueSchema> {
    /// Read all entries in database.
    ///
    /// # Arguments
    /// * `mode` - Reading mode, specified by RocksDB, From start to end, from end to start, or from
    /// arbitrary position to end.
    fn iterator(&self, mode: IteratorMode<S>) -> Result<TezedgeDatabaseIterator<S>, Error>;

    /// Starting from given key, read all entries to the end.
    ///
    /// # Arguments
    /// * `key` - Key (specified by schema), from which to start reading entries
    /// * `max_key_len` - max prefix key length
    fn prefix_iterator(
        &self,
        key: &S::Key,
        max_key_len: usize,
    ) -> Result<TezedgeDatabaseIterator<S>, Error>;
}

pub struct TezedgeDatabaseIterator<S: KeyValueSchema> {
    data: PhantomData<S>,
    inner: Box<BackendIterator>,
}

impl<S: KeyValueSchema> TezedgeDatabaseIterator<S> {
    pub fn new(iter: Box<BackendIterator>) -> Self {
        Self {
            inner: iter,
            data: PhantomData,
        }
    }
}

impl<S: KeyValueSchema> Iterator for TezedgeDatabaseIterator<S> {
    type Item = (Result<S::Key, SchemaError>, Result<S::Value, SchemaError>);

    fn next(&mut self) -> Option<Self::Item> {
        match self.inner.next() {
            Some(i) => match i {
                Ok((k, v)) => Some((S::Key::decode(&k), S::Value::decode(&v))),
                Err(_) => {
                    return None;
                }
            },
            None => {
                return None;
            }
        }
    }
}

pub enum TezedgeDatabaseBackendOptions {
    SledDB(SledDBBackend),
    NotusDB(NotusDBBackend),
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug, PartialEq, Eq, Hash, EnumIter)]
pub enum TezedgeDatabaseBackendConfiguration {
    Sled,
    Notus,
}

impl TezedgeDatabaseBackendConfiguration {
    pub fn possible_values() -> Vec<&'static str> {
        let mut possible_values = Vec::new();
        for sp in TezedgeDatabaseBackendConfiguration::iter() {
            possible_values.extend(sp.supported_values());
        }
        possible_values
    }

    pub fn supported_values(&self) -> Vec<&'static str> {
        match self {
            Self::Sled => vec!["sled"],
            Self::Notus => vec!["notus"],
        }
    }
}

#[derive(Debug, Clone)]
pub struct TezedgeDatabaseBackendConfigurationError(String);

impl FromStr for TezedgeDatabaseBackendConfiguration {
    type Err = TezedgeDatabaseBackendConfigurationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.to_ascii_lowercase();
        for sp in TezedgeDatabaseBackendConfiguration::iter() {
            if sp.supported_values().contains(&s.as_str()) {
                return Ok(sp);
            }
        }

        Err(TezedgeDatabaseBackendConfigurationError(format!(
            "Invalid backend name: {}",
            s
        )))
    }
}

pub trait TezdegeDatabaseBackendKV:
    TezedgeDatabaseBackendStore + TezedgeDatabaseBackendStoreIterator
{
}

pub type TezedgeDatabaseBackend = dyn TezdegeDatabaseBackendKV + Send + Sync;

pub trait TezedgeDatabaseWithIterator<S: KVStoreKeyValueSchema>:
    KVStore<S> + KVStoreWithSchemaIterator<S>
{
}

impl<S: KVStoreKeyValueSchema> TezedgeDatabaseWithIterator<S> for TezedgeDatabase {}

#[derive(Serialize, Deserialize)]
pub struct RWStat {
    pub total: RW,
    pub columns: HashMap<String, RW>,
}
#[derive(Serialize, Deserialize, Clone)]
pub struct RW {
    pub reads: u64,
    pub writes: u64,
    pub ratio: f64,
}

pub struct AtomicRW {
    pub reads: AtomicU64,
    pub writes: AtomicU64,
}

impl Default for AtomicRW {
    fn default() -> Self {
        Self {
            reads: AtomicU64::new(0),
            writes: AtomicU64::new(0),
        }
    }
}

impl AtomicRW {
    pub fn incr_read(&self) {
        self.reads.fetch_add(1, Ordering::Relaxed);
    }

    pub fn incr_write(&self) {
        self.writes.fetch_add(1, Ordering::Relaxed);
    }

    pub fn to_rw(&self) -> RW {
        let reads = self.reads.load(Ordering::Relaxed);
        let writes = self.writes.load(Ordering::Relaxed);
        RW {
            reads,
            writes,
            ratio: reads as f64 / writes as f64,
        }
    }
}

pub struct DatabaseStats {
    pub total: Arc<AtomicRW>,
    pub columns: Arc<HashMap<String, Arc<AtomicRW>>>,
}

impl DatabaseStats {
    pub fn incr_read(&self, column: &'static str) {
        self.total.incr_read();
        let columns = self.columns.clone();
        match columns.get(&column.to_string()) {
            None => {}
            Some(rw) => rw.incr_read(),
        };
    }

    pub fn incr_write(&self, column: &'static str) {
        self.total.incr_write();
        let columns = self.columns.clone();
        match columns.get(&column.to_string()) {
            None => {}
            Some(rw) => {
                rw.incr_write();
            }
        };
    }
    pub fn rw_stats(&self) -> RWStat {
        let columns: HashMap<_, _> = self
            .columns
            .iter()
            .map(|(column, rw)| (column.clone(), rw.to_rw()))
            .collect();

        let rw = self.total.to_rw();
        RWStat {
            total: RW {
                reads: rw.reads,
                writes: rw.writes,
                ratio: rw.reads as f64 / rw.writes as f64,
            },
            columns,
        }
    }
}

impl Default for DatabaseStats {
    fn default() -> Self {
        let mut columns = HashMap::new();
        columns.insert(
            OperationsStorage::column_name().to_string(),
            Arc::new(AtomicRW::default()),
        );
        columns.insert(
            OperationsMetaStorage::column_name().to_string(),
            Arc::new(AtomicRW::default()),
        );
        columns.insert(
            BlockPrimaryIndex::column_name().to_string(),
            Arc::new(AtomicRW::default()),
        );
        columns.insert(
            BlockByLevelIndex::column_name().to_string(),
            Arc::new(AtomicRW::default()),
        );
        columns.insert(
            BlockByContextHashIndex::column_name().to_string(),
            Arc::new(AtomicRW::default()),
        );
        columns.insert(
            BlockMetaStorage::column_name().to_string(),
            Arc::new(AtomicRW::default()),
        );
        columns.insert(
            SystemStorage::column_name().to_string(),
            Arc::new(AtomicRW::default()),
        );
        columns.insert(
            PredecessorStorage::column_name().to_string(),
            Arc::new(AtomicRW::default()),
        );
        columns.insert(
            ChainMetaStorage::column_name().to_string(),
            Arc::new(AtomicRW::default()),
        );
        columns.insert(
            Sequences::column_name().to_string(),
            Arc::new(AtomicRW::default()),
        );

        Self {
            total: Arc::new(AtomicRW::default()),
            columns: Arc::new(columns),
        }
    }
}

pub struct TezedgeDatabase {
    backend: Arc<TezedgeDatabaseBackend>,
    stats: Arc<DatabaseStats>,
}

impl TezedgeDatabase {
    pub fn new(backend_option: TezedgeDatabaseBackendOptions) -> Self {
        match backend_option {
            TezedgeDatabaseBackendOptions::SledDB(backend) => TezedgeDatabase {
                backend: Arc::new(backend),
                stats: Arc::new(DatabaseStats::default()),
            },
            TezedgeDatabaseBackendOptions::NotusDB(backend) => TezedgeDatabase {
                backend: Arc::new(backend),
                stats: Arc::new(DatabaseStats::default()),
            },
        }
    }

    pub fn get_rw_stats(&self) -> RWStat {
        self.stats.rw_stats()
    }

    pub fn flush(&self) -> Result<usize, Error> {
        self.backend.flush()
    }
}

impl<S: KVStoreKeyValueSchema> KVStore<S> for TezedgeDatabase {
    fn put(&self, key: &S::Key, value: &S::Value) -> Result<(), Error> {
        let key = key.encode()?;
        let value = value.encode()?;
        self.backend.put(S::column_name(), key, value)?;
        self.stats.incr_write(S::column_name());
        Ok(())
    }

    fn delete(&self, key: &S::Key) -> Result<(), Error> {
        let key = key.encode()?;
        self.backend.delete(S::column_name(), key)
    }

    fn merge(&self, key: &S::Key, value: &S::Value) -> Result<(), Error> {
        let key = key.encode()?;
        let value = value.encode()?;
        self.backend.merge(S::column_name(), key, value)?;
        self.stats.incr_write(S::column_name());
        Ok(())
    }

    fn get(&self, key: &S::Key) -> Result<Option<S::Value>, Error> {
        let key = key.encode()?;
        let result = self.backend.get(S::column_name(), key);
        self.stats.incr_read(S::column_name());
        match result {
            Ok(value) => match value {
                None => Ok(None),
                Some(value) => Ok(Some(S::Value::decode(value.as_slice())?)),
            },
            Err(error) => Err(error),
        }
    }

    fn contains(&self, key: &S::Key) -> Result<bool, Error> {
        let key = key.encode()?;
        self.backend.contains(S::column_name(), key)
    }

    fn write_batch(&self, batch: Vec<(S::Key, S::Value)>) -> Result<(), Error> {
        let mut generic_batch = Vec::new();

        for (k, v) in batch.iter() {
            generic_batch.push((k.encode()?, v.encode()?))
        }

        self.backend.write_batch(S::column_name(), generic_batch)?;
        self.stats.incr_write(S::column_name());
        Ok(())
    }
}

impl<S: KVStoreKeyValueSchema> KVStoreWithSchemaIterator<S> for TezedgeDatabase {
    fn iterator(&self, mode: IteratorMode<S>) -> Result<TezedgeDatabaseIterator<S>, Error> {
        let mode = match mode {
            IteratorMode::Start => BackendIteratorMode::Start,
            IteratorMode::End => BackendIteratorMode::End,
            IteratorMode::From(key, direction) => {
                BackendIteratorMode::From(key.encode()?, direction)
            }
        };

        let iter = self.backend.iterator(S::column_name(), mode)?;
        Ok(TezedgeDatabaseIterator::new(iter))
    }

    fn prefix_iterator(
        &self,
        key: &<S as KeyValueSchema>::Key,
        max_key_len: usize,
    ) -> Result<TezedgeDatabaseIterator<S>, Error> {
        let key = key.encode()?;
        let iter = self
            .backend
            .prefix_iterator(S::column_name(), &key, max_key_len)?;
        Ok(TezedgeDatabaseIterator::new(iter))
    }
}

#[cfg(test)]
mod tests {}
