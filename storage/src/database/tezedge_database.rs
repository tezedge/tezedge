use crate::database::backend::{BackendIteratorMode, TezedgeDatabaseBackendStore};
use crate::database::error::Error;
use crate::database::rockdb_backend::RocksDBBackend;
use crate::database::sled_backend::SledDBBackend;
use crate::persistent::{Decoder, Encoder, KeyValueSchema, SchemaError};
use crate::IteratorMode;
use serde::{Deserialize, Serialize};
use slog::Logger;
use std::str::FromStr;
use std::sync::Arc;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

use super::backend::BackendIterator;

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
    fn find<'a>(&'a self, mode: IteratorMode<'a, S>) -> Result<BackendIterator<'a>, Error>;

    fn find_by_prefix<'a>(
        &'a self,
        key: &S::Key,
        max_key_len: usize,
    ) -> Result<BackendIterator<'a>, Error>;
}

// TODO - TE-498: Todo Change name
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

pub struct TezedgeDatabase {
    backend: Arc<TezedgeDatabaseBackend>,
    log: slog::Logger,
}

impl TezedgeDatabase {
    pub fn new(backend_option: TezedgeDatabaseBackendOptions, log: Logger) -> Self {
        match backend_option {
            TezedgeDatabaseBackendOptions::SledDB(backend) => TezedgeDatabase {
                backend: Arc::new(backend),
                log: log.clone(),
            },
            TezedgeDatabaseBackendOptions::RocksDB(backend) => TezedgeDatabase {
                backend: Arc::new(backend),
                log: log.clone(),
            },
        }
    }

    fn flush(&self) -> Result<usize, Error> {
        self.backend.flush()
    }

    pub fn flush_checked(&self) {
        match self.flush() {
            Err(e) => {
                slog::error!(&self.log, "Failed to flush database"; "reason" => format!("{:?}", e))
            }
            Ok(_) => slog::info!(&self.log, "Successfully flushed main database"),
        }
    }
}

impl Drop for TezedgeDatabase {
    fn drop(&mut self) {
        self.flush_checked();
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
        Ok(())
    }
}

impl<S: KVStoreKeyValueSchema> KVStoreWithSchemaIterator<S> for TezedgeDatabase {
    fn find<'a>(&'a self, mode: IteratorMode<S>) -> Result<BackendIterator<'a>, Error> {
        let mode = match mode {
            IteratorMode::Start => BackendIteratorMode::Start,
            IteratorMode::End => BackendIteratorMode::End,
            IteratorMode::From(key, direction) => {
                BackendIteratorMode::From(key.encode()?, direction)
            }
        };
        self.backend.find(S::column_name(), mode)
    }

    fn find_by_prefix<'a>(
        &'a self,
        key: &<S as KeyValueSchema>::Key,
        max_key_len: usize,
    ) -> Result<BackendIterator<'a>, Error> {
        let key = key.encode()?;
        self.backend
            .find_by_prefix(S::column_name(), &key, max_key_len)
    }
}

#[cfg(test)]
mod tests {}
