use crate::persistent::{KeyValueSchema};
use crate::database::error::Error;
use crate::IteratorMode;
use crate::persistent::database::IteratorWithSchema;

mod db;
mod error;

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

pub trait DBSubtreeKeyValueSchema : KeyValueSchema {
    fn sub_tree_name() -> &'static str;
}

pub trait KeyValueStoreWithSchemaIterator<S: DBSubtreeKeyValueSchema> {
    /// Read all entries in database.
    ///
    /// # Arguments
    /// * `mode` - Reading mode, specified by RocksDB, From start to end, from end to start, or from
    /// arbitrary position to end.
    fn iterator(&self, mode: IteratorMode<S>) -> Result<IteratorWithSchema<S>, Error>;

    /// Starting from given key, read all entries to the end.
    ///
    /// # Arguments
    /// * `key` - Key (specified by schema), from which to start reading entries
    fn prefix_iterator(&self, key: &S::Key, max_key_len : usize) -> Result<IteratorWithSchema<S>, Error>;
}

/*
impl<S: RocksDbKeyValueSchema> KeyValueStoreWithSchemaIterator<S> for DB {
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
 */