// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::database::backend::{BackendIteratorMode, DBStats, TezedgeDatabaseBackendStore};
use crate::database::error::Error;
use crate::database::tezedge_database::TezdegeDatabaseBackendKV;
use crate::initializer::{RocksDbColumnFactory, RocksDbConfig};
use crate::persistent::database::default_kv_options;
use crate::persistent::DbConfiguration;
use rocksdb::{Cache, ColumnFamilyDescriptor, WriteBatch, WriteOptions, DB};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};
use std::time::Instant;

use super::backend::BackendIterator;

pub struct RocksDBBackend {
    column_stats: Arc<RwLock<HashMap<&'static str, DBStats>>>,
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
        Ok(Self {
            column_stats: Arc::new(Default::default()),
            db,
        })
    }

    pub fn from_db(db: Arc<rocksdb::DB>) -> Result<Self, Error> {
        Ok(Self {
            column_stats: Arc::new(Default::default()),
            db,
        })
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
        let mut stats = self.column_stats.write().map_err(|e| Error::GuardPoison {
            error: format!("{}", e),
        })?;

        let timer = Instant::now();

        let cf = self
            .db
            .cf_handle(column)
            .ok_or(Error::MissingColumnFamily { name: column })?;

        self.db
            .put_cf_opt(cf, key, value, &default_write_options())
            .map_err(Error::from)?;

        let total_write_duration = timer.elapsed();
        let mut stat = stats.entry(column).or_insert(Default::default());
        stat.total_write_duration += total_write_duration;
        stat.total_writes += 1;
        stat.current_write_duration = total_write_duration;
        Ok(())
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
        let mut stats = self.column_stats.write().map_err(|e| Error::GuardPoison {
            error: format!("{}", e),
        })?;

        let timer = Instant::now();

        let cf = self
            .db
            .cf_handle(column)
            .ok_or(Error::MissingColumnFamily { name: column })?;
        let value = self.db.get_cf(cf, key).map_err(Error::from)?;

        let total_read_duration = timer.elapsed();
        let mut stat = stats.entry(column).or_insert(Default::default());
        stat.total_read_duration += total_read_duration;
        stat.total_reads += 1;

        stat.current_read_duration = total_read_duration;
        Ok(value)
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

    fn size(&self) -> HashMap<&'static str, usize> {
        // TODO - TE-721: this doesn't compute anyhting
        HashMap::new()
    }

    fn sync(&self) -> Result<(), Error> {
        todo!()
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

    fn column_stats(&self) -> HashMap<&'static str, DBStats> {
        let stats = match self.column_stats.read().map_err(|e| Error::GuardPoison {
            error: format!("{}", e),
        }) {
            Ok(stats) => stats,
            Err(_) => return Default::default(),
        };
        stats.clone()
    }
}

fn default_write_options() -> WriteOptions {
    let mut opts = WriteOptions::default();
    opts.set_sync(false);
    opts
}
