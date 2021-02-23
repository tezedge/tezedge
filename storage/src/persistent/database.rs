// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::marker::PhantomData;

use failure::Fail;
use rocksdb::{DBIterator, Error, WriteBatch, WriteOptions, DB};
use serde::Serialize;

use crate::persistent::codec::{Decoder, Encoder, SchemaError};
use crate::persistent::schema::KeyValueSchema;
use crypto::hash::FromBytesError;

#[derive(Serialize, Debug, Clone)]
pub struct RocksDBStats {
    pub mem_table_total: u64,
    pub mem_table_unflushed: u64,
    pub mem_table_readers_total: u64,
    pub cache_total: u64,
}

/// Possible errors for schema
#[derive(Debug, Fail)]
pub enum DBError {
    #[fail(display = "Schema error: {}", error)]
    SchemaError { error: SchemaError },
    #[fail(display = "RocksDB error: {}", error)]
    RocksDBError { error: Error },
    #[fail(display = "Column family {} is missing", name)]
    MissingColumnFamily { name: &'static str },
    #[fail(display = "Database incompatibility {}", name)]
    DatabaseIncompatibility { name: String },
    #[fail(display = "Value already exists {}", key)]
    ValueExists { key: String },
    #[fail(display = "Guard Poison {} ", error)]
    GuardPoison { error: String },
    #[fail(display = "SledDB error: {}", error)]
    SledDBError { error: sled::Error },
    #[fail(display = "Hash encode error : {}", error)]
    HashEncodeError { error: FromBytesError },
}

impl From<SchemaError> for DBError {
    fn from(error: SchemaError) -> Self {
        DBError::SchemaError { error }
    }
}

impl From<Error> for DBError {
    fn from(error: Error) -> Self {
        DBError::RocksDBError { error }
    }
}

impl From<FromBytesError> for DBError {
    fn from(error: FromBytesError) -> Self {
        DBError::HashEncodeError { error }
    }
}

impl slog::Value for DBError {
    fn serialize(
        &self,
        _record: &slog::Record,
        key: slog::Key,
        serializer: &mut dyn slog::Serializer,
    ) -> slog::Result {
        serializer.emit_arguments(key, &format_args!("{}", self))
    }
}

impl From<sled::Error> for DBError {
    fn from(error: sled::Error) -> Self {
        DBError::SledDBError { error }
    }
}

pub trait KeyValueStoreWithSchemaIterator<S: KeyValueSchema> {
    /// Read all entries in database.
    ///
    /// # Arguments
    /// * `mode` - Reading mode, specified by RocksDB, From start to end, from end to start, or from
    /// arbitrary position to end.
    fn iterator(&self, mode: IteratorMode<S>) -> Result<IteratorWithSchema<S>, DBError>;

    /// Starting from given key, read all entries to the end.
    ///
    /// # Arguments
    /// * `key` - Key (specified by schema), from which to start reading entries
    fn prefix_iterator(&self, key: &S::Key) -> Result<IteratorWithSchema<S>, DBError>;
}

/// Custom trait extending RocksDB to better handle and enforce database schema
pub trait KeyValueStoreBackend<S: KeyValueSchema> {
    /// Provides informatio if backend is persistent
    fn is_persistent(&self) -> bool;

    /// Insert new key value pair into the database.
    ///
    /// # Arguments
    /// * `key` - Value of key specified by schema
    /// * `value` - Value to be inserted associated with given key, specified by schema
    fn put(&self, key: &S::Key, value: &S::Value) -> Result<(), DBError>;

    /// Delete existing value associated with given key from the database.
    ///
    /// # Arguments
    /// * `key` - Value of key specified by schema
    fn delete(&self, key: &S::Key) -> Result<(), DBError>;

    /// Delete existing value associated with given key from the database.
    ///
    /// # Arguments
    /// * `key` - Value of key specified by schema
    fn try_delete(&self, key: &S::Key) -> Result<Option<S::Value>, DBError> {
        let v = self.get(key)?;
        if v.is_some() {
            self.delete(key)?;
        }
        Ok(v)
    }

    /// Insert key value pair into the database, overriding existing value if exists.
    ///
    /// # Arguments
    /// * `key` - Value of key specified by schema
    /// * `value` - Value to be inserted associated with given key, specified by schema
    fn merge(&self, key: &S::Key, value: &S::Value) -> Result<(), DBError>;

    /// Read value associated with given key, if exists.
    ///
    /// # Arguments
    /// * `key` - Value of key specified by schema
    fn get(&self, key: &S::Key) -> Result<Option<S::Value>, DBError>;

    /// Check, if database contains given key
    ///
    /// # Arguments
    /// * `key` - Key (specified by schema), to be checked for existence
    fn contains(&self, key: &S::Key) -> Result<bool, DBError>;

    /// Check, if database contains given key
    ///
    /// # Arguments
    /// * `key` - Key (specified by schema), to be checked for existence
    fn retain(&self, predicate: &dyn Fn(&S::Key) -> bool) -> Result<(), DBError>;

    /// Write batch into DB atomically
    ///
    /// # Arguments
    /// * `batch` - WriteBatch containing all batched writes to be written to DB
    fn write_batch(&self, batch: Vec<(S::Key, S::Value)>) -> Result<(), DBError>;

    /// Return memory usage statistics
    ///
    fn total_get_mem_usage(&self) -> Result<usize, DBError>;
}

pub trait KeyValueStoreWithSchema<S: KeyValueSchema>:
    KeyValueStoreBackend<S> + KeyValueStoreWithSchemaIterator<S>
{
}

impl<S: KeyValueSchema> KeyValueStoreWithSchemaIterator<S> for DB {
    fn iterator(&self, mode: IteratorMode<S>) -> Result<IteratorWithSchema<S>, DBError> {
        let cf = self
            .cf_handle(S::name())
            .ok_or(DBError::MissingColumnFamily { name: S::name() })?;

        let iter = match mode {
            IteratorMode::Start => self.iterator_cf(cf, rocksdb::IteratorMode::Start),
            IteratorMode::End => self.iterator_cf(cf, rocksdb::IteratorMode::End),
            IteratorMode::From(key, direction) => self.iterator_cf(
                cf,
                rocksdb::IteratorMode::From(&key.encode()?, direction.into()),
            ),
        };

        Ok(IteratorWithSchema(iter, PhantomData))
    }

    fn prefix_iterator(&self, key: &S::Key) -> Result<IteratorWithSchema<S>, DBError> {
        let key = key.encode()?;
        let cf = self
            .cf_handle(S::name())
            .ok_or(DBError::MissingColumnFamily { name: S::name() })?;

        Ok(IteratorWithSchema(
            self.prefix_iterator_cf(cf, key),
            PhantomData,
        ))
    }
}

impl<S: KeyValueSchema> KeyValueStoreWithSchema<S> for DB {}

impl<S: KeyValueSchema> KeyValueStoreBackend<S> for DB {
    fn is_persistent(&self) -> bool {
        true
    }

    fn put(&self, key: &S::Key, value: &S::Value) -> Result<(), DBError> {
        let key = key.encode()?;
        let value = value.encode()?;
        let cf = self
            .cf_handle(S::name())
            .ok_or(DBError::MissingColumnFamily { name: S::name() })?;

        self.put_cf_opt(cf, &key, &value, &default_write_options())
            .map_err(DBError::from)
    }

    fn delete(&self, key: &S::Key) -> Result<(), DBError> {
        let key = key.encode()?;
        let cf = self
            .cf_handle(S::name())
            .ok_or(DBError::MissingColumnFamily { name: S::name() })?;

        self.delete_cf_opt(cf, &key, &default_write_options())
            .map_err(DBError::from)
    }

    fn merge(&self, key: &S::Key, value: &S::Value) -> Result<(), DBError> {
        let key = key.encode()?;
        let value = value.encode()?;
        let cf = self
            .cf_handle(S::name())
            .ok_or(DBError::MissingColumnFamily { name: S::name() })?;

        self.merge_cf_opt(cf, &key, &value, &default_write_options())
            .map_err(DBError::from)
    }

    fn get(&self, key: &S::Key) -> Result<Option<S::Value>, DBError> {
        let key = key.encode()?;
        let cf = self
            .cf_handle(S::name())
            .ok_or(DBError::MissingColumnFamily { name: S::name() })?;

        self.get_cf(cf, &key)
            .map_err(DBError::from)?
            .map(|value| S::Value::decode(&value))
            .transpose()
            .map_err(DBError::from)
    }

    fn contains(&self, key: &S::Key) -> Result<bool, DBError> {
        let key = key.encode()?;
        let cf = self
            .cf_handle(S::name())
            .ok_or(DBError::MissingColumnFamily { name: S::name() })?;

        let val = self.get_pinned_cf(cf, &key)?;
        Ok(val.is_some())
    }

    fn write_batch(&self, batch: Vec<(S::Key, S::Value)>) -> Result<(), DBError> {
        let mut rocksb_batch = WriteBatch::default(); // batch containing DB key values to persist

        for (k, v) in batch.iter() {
            let key = k.encode()?;
            let value = v.encode()?;
            let cf = self
                .cf_handle(S::name())
                .ok_or(DBError::MissingColumnFamily { name: S::name() })?;
            rocksb_batch.put_cf(cf, &key, &value);
        }

        self.write_opt(rocksb_batch, &default_write_options())?;
        Ok(())
    }

    fn total_get_mem_usage(&self) -> Result<usize, DBError> {
        let memory_usage_stats = rocksdb::perf::get_memory_usage_stats(Some(&[&self]), None)?;
        return Ok((memory_usage_stats.mem_table_total
            + memory_usage_stats.mem_table_unflushed
            + memory_usage_stats.mem_table_readers_total
            + memory_usage_stats.cache_total) as usize);
    }

    fn retain(&self, predicate: &dyn Fn(&S::Key) -> bool) -> Result<(), DBError> {
        let garbage: Vec<_> = (self as &dyn KeyValueStoreWithSchemaIterator<S>)
            .iterator(IteratorMode::Start)
            .unwrap()
            .filter_map(|(k, _)| match k {
                Err(_) => None,
                Ok(value) => {
                    if !predicate(&value) {
                        Some(value)
                    } else {
                        None
                    }
                }
            })
            .collect();

        for i in garbage {
            (self as &dyn KeyValueStoreBackend<S>).delete(&i)?;
        }
        Ok(())
    }
}

fn default_write_options() -> WriteOptions {
    let mut opts = WriteOptions::default();
    opts.set_sync(false);
    opts
}

/// Database iterator extended by specific schema
pub struct IteratorWithSchema<'a, S: KeyValueSchema>(DBIterator<'a>, PhantomData<S>);

impl<'a, S: KeyValueSchema> Iterator for IteratorWithSchema<'a, S> {
    type Item = (Result<S::Key, SchemaError>, Result<S::Value, SchemaError>);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0
            .next()
            .map(|(k, v)| (S::Key::decode(&k), S::Value::decode(&v)))
    }
}

/// Database iterator direction
pub enum Direction {
    Forward,
    Reverse,
}

impl From<Direction> for rocksdb::Direction {
    fn from(direction: Direction) -> Self {
        match direction {
            Direction::Forward => rocksdb::Direction::Forward,
            Direction::Reverse => rocksdb::Direction::Reverse,
        }
    }
}

/// Database iterator with schema mode, from start to end, from end to start or from specific key to end/start
pub enum IteratorMode<'a, S: KeyValueSchema> {
    Start,
    End,
    From(&'a S::Key, Direction),
}
