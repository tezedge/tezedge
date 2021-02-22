// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::merkle_storage::{ContextValue, EntryHash};
use crate::persistent::database::GetInMemStats;
use crate::persistent::database::KeyValueStoreWithSchema;
use crate::persistent::database::RocksDBStats;
use crate::storage_backend::{StorageBackend, StorageBackendError};
use crate::MerkleStorage;
use rocksdb::WriteBatch;
use rocksdb::{WriteOptions, DB};
use serde::{Deserialize, Serialize};
use std::ops::Deref;
use std::sync::Arc;

pub struct RocksDBBackend {
    column_name: &'static str,
    inner: Arc<DB>,
}

impl RocksDBBackend {
    pub fn new(db: Arc<DB>, column_name: &'static str) -> Self {
        RocksDBBackend {
            inner: db,
            column_name,
        }
    }
}

impl RocksDBBackend {
    fn default_write_options() -> WriteOptions {
        let mut opts = WriteOptions::default();
        opts.set_sync(false);
        opts
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RocksDBBackendStats {
    mem_table_total: u64,
    mem_table_unflushed: u64,
    mem_table_readers_total: u64,
    cache_total: u64,
}

impl StorageBackend for RocksDBBackend {
    fn is_persisted(&self) -> bool {
        true
    }

    fn put(&mut self, key: &EntryHash, value: ContextValue) -> Result<bool, StorageBackendError> {
        let cf = self.inner.cf_handle(&self.column_name).ok_or(
            StorageBackendError::MissingColumnFamily {
                name: &self.column_name,
            },
        )?;

        self.inner
            .put_cf_opt(
                cf,
                &key.as_ref()[..],
                &value,
                &Self::default_write_options(),
            )
            .map_err(StorageBackendError::from)
            .map(|_| true)
    }

    fn put_batch(
        &mut self,
        batch: Vec<(EntryHash, ContextValue)>,
    ) -> Result<(), StorageBackendError> {
        let mut rocksb_batch = WriteBatch::default(); // batch containing DB key values to persist

        for (k, v) in batch.iter() {
            (self.inner.deref() as &dyn KeyValueStoreWithSchema<MerkleStorage>).put_batch(
                &mut rocksb_batch,
                &k,
                &v,
            )?;
        }

        (self.inner.deref() as &dyn KeyValueStoreWithSchema<MerkleStorage>)
            .write_batch(rocksb_batch)?;

        Ok(())
    }

    fn merge(&mut self, key: &EntryHash, value: ContextValue) -> Result<(), StorageBackendError> {
        let cf = self.inner.cf_handle(&self.column_name).ok_or(
            StorageBackendError::MissingColumnFamily {
                name: &self.column_name,
            },
        )?;

        self.inner
            .merge_cf_opt(
                cf,
                &key.as_ref()[..],
                &value,
                &Self::default_write_options(),
            )
            .map_err(StorageBackendError::from)
    }

    /// Warning: always returns None.
    fn delete(&mut self, key: &EntryHash) -> Result<Option<ContextValue>, StorageBackendError> {
        let cf = self.inner.cf_handle(&self.column_name).ok_or(
            StorageBackendError::MissingColumnFamily {
                name: &self.column_name,
            },
        )?;

        self.inner
            .delete_cf_opt(cf, &key.as_ref()[..], &Self::default_write_options())
            .map_err(StorageBackendError::from)?;
        Ok(None)
    }

    fn get(&self, key: &EntryHash) -> Result<Option<ContextValue>, StorageBackendError> {
        let cf = self.inner.cf_handle(&self.column_name).ok_or(
            StorageBackendError::MissingColumnFamily {
                name: &self.column_name,
            },
        )?;

        let v = self
            .inner
            .get_cf(cf, &key.as_ref()[..])
            .map_err(StorageBackendError::from)?;
        Ok(v)
    }

    fn contains(&self, key: &EntryHash) -> Result<bool, StorageBackendError> {
        self.get(key).map(|v| v.is_some())
    }

    fn get_mem_use_stats(&self) -> Result<RocksDBStats, StorageBackendError> {
        self.inner
            .get_stats()
            .map_err(|_| StorageBackendError::BackendError)
    }
}
