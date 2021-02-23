// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::merkle_storage::{ContextValue, EntryHash};
use std::ops::Deref;
use crypto::hash::HashType;
use crate::MerkleStorage;
use rocksdb::WriteBatch;
use crate::storage_backend::{StorageBackend, StorageBackendError};
use crate::persistent::database::{SimpleKeyValueStoreWithSchema, DBError, RocksDBStats};

pub struct SledBackend {
    db: sled::Db,
    inner: sled::Tree,
}

impl SledBackend {
    pub fn new(db: sled::Db) -> Self {
        SledBackend {
            inner: db.deref().clone(),
            db,
        }
    }
}

impl SimpleKeyValueStoreWithSchema<MerkleStorage> for SledBackend {

    fn put(& self, key: &EntryHash, value: &ContextValue) -> Result<(), DBError> {
        if self.inner.contains_key(key)?{
            Err(DBError::ValueExists{key: HashType::ContextHash.hash_to_b58check(key)?})
        }else{
            self
                .inner
                .insert(&key.as_ref()[..], value.clone())?;
            Ok(())
        }
    }

    fn delete(&self, key: &EntryHash) -> Result<(), DBError> {
        self.inner.remove(&key.as_ref()[..])?;
        Ok(())
    }

    fn merge(&self, key: &EntryHash, value: &ContextValue) -> Result<(), DBError> {
        self.inner.insert(&key.as_ref()[..], value.clone())?;
        Ok(())
    }

    fn get(&self, key: &EntryHash) -> Result<Option<ContextValue>, DBError> {
        Ok(self.inner.get(&key.as_ref()[..])?.map(|ivec| ivec.to_vec()))
    }

    fn contains(&self, key: &EntryHash) -> Result<bool, DBError> {
        Ok(self.inner.contains_key(&key.as_ref()[..])?)
    }

    fn put_batch(
        &self,
        batch: &mut WriteBatch,
        key: &EntryHash,
        value: &ContextValue,
    ) -> Result<(), DBError> {
        unimplemented!();
    }

    fn write_batch(&self, batch: WriteBatch) -> Result<(), DBError> {
        unimplemented!();
    }

    fn get_stats(&self) -> Result<RocksDBStats, DBError> {
        unimplemented!();
    }

}

impl StorageBackend for SledBackend {
    fn is_persisted(&self) -> bool {
        true
    }

    fn put(&mut self, key: &EntryHash, value: ContextValue) -> Result<bool, StorageBackendError> {
        Ok(self
            .inner
            .insert(&key.as_ref()[..], value)
            .map(|v| v.is_none())?)
    }

    fn merge(&mut self, key: &EntryHash, value: ContextValue) -> Result<(), StorageBackendError> {
        self.inner.insert(&key.as_ref()[..], value)?;
        Ok(())
    }

    fn delete(&mut self, key: &EntryHash) -> Result<Option<ContextValue>, StorageBackendError> {
        Ok(self.inner.remove(&key.as_ref()[..])?.map(|v| v.to_vec()))
    }

    fn get(&self, key: &EntryHash) -> Result<Option<ContextValue>, StorageBackendError> {
        let r = self.inner.get(&key.as_ref()[..])?;

        match r {
            None => Err(StorageBackendError::BackendError),
            Some(v) => Ok(Some(v.to_vec())),
        }
    }

    fn contains(&self, key: &EntryHash) -> Result<bool, StorageBackendError> {
        Ok(self.inner.contains_key(&key.as_ref()[..])?)
    }

    fn total_get_mem_usage(&self) -> Result<usize,StorageBackendError> {
        self.db.size_on_disk()
            .map(|size| size as usize)
            .map_err(|e| StorageBackendError::SledDBError{error: e})
    }
}
