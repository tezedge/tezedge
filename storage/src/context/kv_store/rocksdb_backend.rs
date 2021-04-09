// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::ops::Deref;
use std::sync::Arc;

use rocksdb::{Cache, ColumnFamilyDescriptor, DB};
use serde::{Deserialize, Serialize};

use crate::context::gc::NotGarbageCollected;
use crate::context::merkle::hash::EntryHash;
use crate::context::{
    ContextKeyValueStoreSchema, ContextKeyValueStoreSchemaKeyType, ContextValue,
    MerkleKeyValueStoreSchemaValueType,
};
use crate::persistent::database::{default_table_options, DBError, RocksDbKeyValueSchema};
use crate::persistent::{
    Flushable, KeyValueSchema, KeyValueStoreBackend, MultiInstanceable, MultiInstanceableSyncError,
    Persistable,
};

impl KeyValueSchema for RocksDBBackend {
    type Key = ContextKeyValueStoreSchemaKeyType;
    type Value = MerkleKeyValueStoreSchemaValueType;
}

impl RocksDbKeyValueSchema for RocksDBBackend {
    fn descriptor(cache: &Cache) -> ColumnFamilyDescriptor {
        let cf_opts = default_table_options(cache);
        ColumnFamilyDescriptor::new(Self::name(), cf_opts)
    }

    #[inline]
    fn name() -> &'static str {
        "merkle_storage"
    }
}

pub struct RocksDBBackend {
    inner: Arc<DB>,
}

impl RocksDBBackend {
    pub fn new(db: Arc<DB>) -> Self {
        RocksDBBackend { inner: db }
    }

    // TODO TE-437 - get rid of deref call
    fn merkle_ref(&self) -> &dyn KeyValueStoreBackend<RocksDBBackend> {
        self.inner.deref() as &dyn KeyValueStoreBackend<RocksDBBackend>
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RocksDBBackendStats {
    mem_table_total: u64,
    mem_table_unflushed: u64,
    mem_table_readers_total: u64,
    cache_total: u64,
}

impl NotGarbageCollected for RocksDBBackend {}

impl KeyValueStoreBackend<ContextKeyValueStoreSchema> for RocksDBBackend {
    fn put(&self, key: &EntryHash, value: &ContextValue) -> Result<(), DBError> {
        self.merkle_ref().put(key, value)
    }

    fn delete(&self, key: &EntryHash) -> Result<(), DBError> {
        self.merkle_ref().delete(key)
    }

    fn merge(&self, key: &EntryHash, value: &ContextValue) -> Result<(), DBError> {
        self.merkle_ref().merge(key, value)
    }

    fn get(&self, key: &EntryHash) -> Result<Option<ContextValue>, DBError> {
        self.merkle_ref().get(key)
    }

    fn contains(&self, key: &EntryHash) -> Result<bool, DBError> {
        self.merkle_ref().contains(key)
    }

    fn retain(&self, predicate: &dyn Fn(&EntryHash) -> bool) -> Result<(), DBError> {
        self.merkle_ref().retain(predicate)
    }

    fn write_batch(&self, batch: Vec<(EntryHash, ContextValue)>) -> Result<(), DBError> {
        self.merkle_ref().write_batch(batch)
    }

    fn total_get_mem_usage(&self) -> Result<usize, DBError> {
        self.merkle_ref().total_get_mem_usage()
    }
}

impl Flushable for RocksDBBackend {
    fn flush(&self) -> Result<(), failure::Error> {
        match self.inner.flush() {
            Ok(_) => Ok(()),
            Err(e) => Err(failure::format_err!(
                "Failed to flush rocksdb for context, reason: {:?}",
                e
            )),
        }
    }
}

impl MultiInstanceable for RocksDBBackend {
    fn supports_multiple_opened_instances(&self) -> bool {
        true
    }

    fn sync_with_primary(&self) -> Result<(), MultiInstanceableSyncError> {
        // TODO: TE-150 - real support mutliprocess
        self.inner
            .try_catch_up_with_primary()
            .map_err(|e| MultiInstanceableSyncError::new(format!("{:?}", e)))
    }
}

impl Persistable for RocksDBBackend {
    fn is_persistent(&self) -> bool {
        true
    }
}
