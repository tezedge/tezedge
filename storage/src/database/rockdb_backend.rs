use crate::database::backend::{BackendIteratorMode, TezedgeDatabaseBackendStore};
use crate::database::error::Error;
use crate::database::tezedge_database::TezdegeDatabaseBackendKV;
use crate::initializer::{RocksDbColumnFactory, RocksDbConfig};
use crate::persistent::database::default_kv_options;
use crate::persistent::DbConfiguration;
use rocksdb::{Cache, ColumnFamilyDescriptor, WriteBatch, WriteOptions, DB};
use std::path::Path;
use std::sync::Arc;

use super::backend::BackendIterator;

pub struct RocksDBBackend {
    db: Arc<rocksdb::DB>,
}

impl RocksDBBackend {
    pub fn new<C: RocksDbColumnFactory>(
        cache: &Cache,
        config: &RocksDbConfig<C>,
    ) -> Result<Self, Error> {
        let db = Self::open_kv(
            &config.db_path,
            config.columns.create(cache),
            &DbConfiguration {
                max_threads: config.threads,
            },
        )
        .map(Arc::new)?;
        Ok(Self { db })
    }

    pub fn from_db(db: Arc<rocksdb::DB>) -> Result<Self, Error> {
        Ok(Self { db })
    }

    fn open_kv<P, I>(path: P, cfs: I, cfg: &DbConfiguration) -> Result<DB, Error>
    where
        P: AsRef<Path>,
        I: IntoIterator<Item = ColumnFamilyDescriptor>,
    {
        DB::open_cf_descriptors(&default_kv_options(cfg), path, cfs).map_err(Error::from)
    }
}
impl TezdegeDatabaseBackendKV for RocksDBBackend {}
impl TezedgeDatabaseBackendStore for RocksDBBackend {
    fn put(&self, column: &'static str, key: &[u8], value: &[u8]) -> Result<(), Error> {
        let cf = self
            .db
            .cf_handle(column)
            .ok_or(Error::MissingColumnFamily { name: column })?;

        self.db
            .put_cf_opt(cf, key, value, &default_write_options())
            .map_err(Error::from)
    }

    fn delete(&self, column: &'static str, key: &[u8]) -> Result<(), Error> {
        let cf = self
            .db
            .cf_handle(column)
            .ok_or(Error::MissingColumnFamily { name: column })?;
        self.db
            .delete_cf_opt(cf, key, &default_write_options())
            .map_err(Error::from)
    }

    fn merge(&self, column: &'static str, key: &[u8], value: &[u8]) -> Result<(), Error> {
        let cf = self
            .db
            .cf_handle(column)
            .ok_or(Error::MissingColumnFamily { name: column })?;

        self.db
            .merge_cf_opt(cf, key, value, &default_write_options())
            .map_err(Error::from)
    }

    fn get(&self, column: &'static str, key: &[u8]) -> Result<Option<Vec<u8>>, Error> {
        let cf = self
            .db
            .cf_handle(column)
            .ok_or(Error::MissingColumnFamily { name: column })?;
        self.db.get_cf(cf, key).map_err(Error::from)
    }

    fn contains(&self, column: &'static str, key: &[u8]) -> Result<bool, Error> {
        let cf = self
            .db
            .cf_handle(column)
            .ok_or(Error::MissingColumnFamily { name: column })?;
        let val = self.db.get_pinned_cf(cf, key)?;
        Ok(val.is_some())
    }

    fn write_batch(
        &self,
        column: &'static str,
        batch: Vec<(Vec<u8>, Vec<u8>)>,
    ) -> Result<(), Error> {
        let mut rocksb_batch = WriteBatch::default();
        for (key, value) in batch.iter() {
            let cf = self
                .db
                .cf_handle(column)
                .ok_or(Error::MissingColumnFamily { name: column })?;
            rocksb_batch.put_cf(cf, &key, &value);
        }
        self.db.write_opt(rocksb_batch, &default_write_options())?;
        Ok(())
    }

    fn flush(&self) -> Result<usize, Error> {
        self.db.flush()?;
        Ok(0)
    }

    fn find<'a>(
        &'a self,
        column: &'static str,
        mode: BackendIteratorMode,
    ) -> Result<BackendIterator<'a>, Error> {
        let cf = self
            .db
            .cf_handle(column)
            .ok_or(Error::MissingColumnFamily { name: column })?;

        let iter = match mode {
            BackendIteratorMode::Start => self.db.iterator_cf(cf, rocksdb::IteratorMode::Start),
            BackendIteratorMode::End => self.db.iterator_cf(cf, rocksdb::IteratorMode::End),
            BackendIteratorMode::From(key, direction) => self
                .db
                .iterator_cf(cf, rocksdb::IteratorMode::From(&key, direction.into())),
        };

        Ok(Box::new(iter.map(|kv| Ok(kv))))
    }

    fn find_by_prefix<'a>(
        &'a self,
        column: &'static str,
        key: &Vec<u8>,
        _: usize,
    ) -> Result<BackendIterator<'a>, Error> {
        let cf = self
            .db
            .cf_handle(column)
            .ok_or(Error::MissingColumnFamily { name: column })?;

        Ok(Box::new(
            self.db.prefix_iterator_cf(cf, key).map(|kv| Ok(kv)),
        ))
    }
}

fn default_write_options() -> WriteOptions {
    let mut opts = WriteOptions::default();
    opts.set_sync(false);
    opts
}
