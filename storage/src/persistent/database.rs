// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::marker::PhantomData;

use failure::Fail;
use rocksdb::{DBIterator, Error, WriteBatch, WriteOptions, DB};
use serde::Serialize;

use crate::persistent::codec::{Decoder, Encoder, SchemaError};
use crate::persistent::schema::KeyValueSchema;

#[derive(Serialize, Debug, Clone)]
pub struct RocksDBStats {
    mem_table_total: u64,
    mem_table_unflushed: u64,
    mem_table_readers_total: u64,
    cache_total: u64,
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

/// Custom trait extending RocksDB to better handle and enforce database schema
pub trait KeyValueStoreWithSchema<S: KeyValueSchema> {
    /// Insert new key value pair into the database. If key already exists, method will fail
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

    /// Check, if database contains given key
    ///
    /// # Arguments
    /// * `key` - Key (specified by schema), to be checked for existence
    fn contains(&self, key: &S::Key) -> Result<bool, DBError>;

    /// Insert new key value pair into WriteBatch.
    ///
    /// # Arguments
    /// * `key` - Value of key specified by schema
    /// * `value` - Value to be inserted associated with given key, specified by schema
    fn put_batch(
        &self,
        batch: &mut WriteBatch,
        key: &S::Key,
        value: &S::Value,
    ) -> Result<(), DBError>;

    /// Write batch into DB atomically
    ///
    /// # Arguments
    /// * `batch` - WriteBatch containing all batched writes to be written to DB
    fn write_batch(&self, batch: WriteBatch) -> Result<(), DBError>;

    /// Get memory usage statistics from DB
    fn get_mem_use_stats(&self) -> Result<RocksDBStats, DBError>;
}

impl<S: KeyValueSchema> KeyValueStoreWithSchema<S> for DB {
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

    fn contains(&self, key: &S::Key) -> Result<bool, DBError> {
        let key = key.encode()?;
        let cf = self
            .cf_handle(S::name())
            .ok_or(DBError::MissingColumnFamily { name: S::name() })?;

        let val = self.get_pinned_cf(cf, &key)?;
        Ok(val.is_some())
    }

    fn put_batch(
        &self,
        batch: &mut WriteBatch,
        key: &S::Key,
        value: &S::Value,
    ) -> Result<(), DBError> {
        let key = key.encode()?;
        let value = value.encode()?;
        let cf = self
            .cf_handle(S::name())
            .ok_or(DBError::MissingColumnFamily { name: S::name() })?;

        batch.put_cf(cf, &key, &value);

        Ok(())
    }

    fn write_batch(&self, batch: WriteBatch) -> Result<(), DBError> {
        self.write_opt(batch, &default_write_options())?;
        Ok(())
    }

    fn get_mem_use_stats(&self) -> Result<RocksDBStats, DBError> {
        let memory_usage_stats = rocksdb::perf::get_memory_usage_stats(Some(&[&self]), None)?;

        Ok(RocksDBStats {
            mem_table_total: memory_usage_stats.mem_table_total,
            mem_table_unflushed: memory_usage_stats.mem_table_unflushed,
            mem_table_readers_total: memory_usage_stats.mem_table_readers_total,
            cache_total: memory_usage_stats.cache_total,
        })
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
