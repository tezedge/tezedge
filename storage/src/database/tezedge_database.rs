use crate::block_storage::{BlockByContextHashIndex, BlockByLevelIndex, BlockPrimaryIndex};
use crate::database::backend::{BackendIteratorMode, TezedgeDatabaseBackendStore};
use crate::database::error::Error;
use crate::database::rockdb_backend::RocksDBBackend;
use crate::database::sled_backend::SledDBBackend;
use crate::persistent::sequence::Sequences;
use crate::persistent::{Decoder, Encoder, KeyValueSchema, SchemaError};
use crate::{BlockMetaStorage, ChainMetaStorage, OperationsMetaStorage, OperationsStorage, PredecessorStorage, SystemStorage, IteratorMode};
use im::HashMap;
use serde::{Deserialize, Serialize};
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
    fn find(
        &self,
        mode: IteratorMode<S>,
        limit: Option<usize>,
        filter: Box<dyn Fn((&[u8], &[u8])) -> Result<bool, SchemaError>>,
    ) -> Result<Vec<(Box<[u8]>, Box<[u8]>)>, Error>;

    fn find_by_prefix(
        &self,
        key: &S::Key,
        max_key_len: usize,
        filter: Box<dyn Fn((&[u8], &[u8])) -> Result<bool, SchemaError>>,
    ) -> Result<Vec<(Box<[u8]>, Box<[u8]>)>, Error>;
}

//Todo Change name
pub type List<S> = Vec<(
    Result<<S as KeyValueSchema>::Key, SchemaError>,
    Result<<S as KeyValueSchema>::Value, SchemaError>,
)>;

pub enum TezedgeDatabaseBackendOptions {
    SledDB(SledDBBackend),
    RocksDB(RocksDBBackend),
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug, PartialEq, Eq, Hash, EnumIter)]
pub enum TezedgeDatabaseBackendConfiguration {
    Sled,
    RocksDB,
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
            Self::RocksDB => vec!["rocksdb"],
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

pub trait TezdegeDatabaseBackendKV: TezedgeDatabaseBackendStore {}

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
            TezedgeDatabaseBackendOptions::RocksDB(backend) => TezedgeDatabase {
                backend: Arc::new(backend),
                stats: Arc::new(Default::default()),
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
        self.backend.put(S::column_name(), &key, &value)?;
        Ok(())
    }

    fn delete(&self, key: &S::Key) -> Result<(), Error> {
        let key = key.encode()?;
        self.backend.delete(S::column_name(), &key)
    }

    fn merge(&self, key: &S::Key, value: &S::Value) -> Result<(), Error> {
        let key = key.encode()?;
        let value = value.encode()?;
        self.backend.merge(S::column_name(), &key, &value)?;
        Ok(())
    }

    fn get(&self, key: &S::Key) -> Result<Option<S::Value>, Error> {
        let key = key.encode()?;
        let result = self.backend.get(S::column_name(), &key);
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
        self.backend.contains(S::column_name(), &key)
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
    fn find(
        &self,
        mode: IteratorMode<S>,
        limit: Option<usize>,
        filter: Box<dyn Fn((&[u8], &[u8])) -> Result<bool, SchemaError>>,
    ) -> Result<Vec<(Box<[u8]>, Box<[u8]>)>, Error> {
        let mode = match mode {
            IteratorMode::Start => BackendIteratorMode::Start,
            IteratorMode::End => BackendIteratorMode::End,
            IteratorMode::From(key, direction) => {
                BackendIteratorMode::From(key.encode()?, direction)
            }
        };
        self.backend.find(S::column_name(), mode, limit, filter)
    }

    fn find_by_prefix(
        &self,
        key: &<S as KeyValueSchema>::Key,
        max_key_len: usize,
        filter: Box<dyn Fn((&[u8], &[u8])) -> Result<bool, SchemaError>>,
    ) -> Result<Vec<(Box<[u8]>, Box<[u8]>)>, Error> {
        let key = key.encode()?;
        self.backend
            .find_by_prefix(S::column_name(), &key, max_key_len, filter)
    }
}

#[cfg(test)]
mod tests {}
