use crate::database::error::Error;
use crate::persistent::{KeyValueSchema, SchemaError};
use crate::{Direction, IteratorMode};

use crate::persistent::codec::Decoder;
use sled::{IVec, Tree};
use std::marker::PhantomData;
use std::sync::Arc;

pub mod db;
pub mod error;

/// Custom trait to unify any kv-store schema access
pub trait KVDatabase<S: KeyValueSchema> {
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

pub trait DBSubtreeKeyValueSchema: KeyValueSchema {
    fn sub_tree_name() -> &'static str;
}

pub trait KVDatabaseWithSchemaIterator<S: DBSubtreeKeyValueSchema> {
    /// Read all entries in database.
    ///
    /// # Arguments
    /// * `mode` - Reading mode, specified by RocksDB, From start to end, from end to start, or from
    /// arbitrary position to end.
    fn iterator(&self, mode: IteratorMode<S>) -> Result<KVDBIteratorWithSchema<S>, Error>;

    /// Starting from given key, read all entries to the end.
    ///
    /// # Arguments
    /// * `key` - Key (specified by schema), from which to start reading entries
    /// * `max_key_len` - max prefix key length
    fn prefix_iterator(
        &self,
        key: &S::Key,
        max_key_len: usize,
    ) -> Result<KVDBIteratorWithSchema<S>, Error>;
}

pub struct KVDBIteratorWithSchema<S: KeyValueSchema>(SledIteratorWrapper, PhantomData<S>);

pub trait KVDBStoreWithSchema<S: DBSubtreeKeyValueSchema>:
    KVDatabase<S> + KVDatabaseWithSchemaIterator<S>
{
}

#[derive(Clone)]
pub enum SledIteratorWrapperMode {
    Start,
    End,
    From(IVec, Direction),
    Prefix(IVec),
}
pub struct SledIteratorWrapper {
    mode: SledIteratorWrapperMode,
    iter: sled::Iter,
}

impl SledIteratorWrapper {
    fn new(mode: SledIteratorWrapperMode, tree: Tree) -> Self {
        match mode.clone() {
            SledIteratorWrapperMode::Start => Self {
                mode,
                iter: tree.iter(),
            },
            SledIteratorWrapperMode::End => Self {
                mode,
                iter: tree.iter(),
            },
            SledIteratorWrapperMode::From(key, direction) => {
                let iter = match direction {
                    Direction::Forward => tree.range(key..),
                    Direction::Reverse => tree.range(..=key),
                };

                Self { mode, iter }
            }
            SledIteratorWrapperMode::Prefix(key) => Self {
                mode,
                iter: tree.scan_prefix(key),
            },
        }
    }
}

impl Iterator for SledIteratorWrapper {
    type Item = Result<(IVec, IVec), sled::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        match &self.mode {
            SledIteratorWrapperMode::Start => self.iter.next(),
            SledIteratorWrapperMode::End => self.iter.next_back(),
            SledIteratorWrapperMode::From(_, direction) => match direction {
                Direction::Forward => self.iter.next(),
                Direction::Reverse => self.iter.next_back(),
            },
            SledIteratorWrapperMode::Prefix(_) => self.iter.next(),
        }
    }
}
impl<S: KeyValueSchema> Iterator for KVDBIteratorWithSchema<S> {
    type Item = (Result<S::Key, SchemaError>, Result<S::Value, SchemaError>);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self.0.next() {
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
