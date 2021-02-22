// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::merkle_storage::{ContextValue, EntryHash};
use crate::persistent::database::{DBError, KeyValueStoreBackend};
use crate::storage_backend::{GarbageCollector, StorageBackendError};
use crate::MerkleStorage;
use bytes::Buf;
use std::io::Read;
use std::ops::Deref;

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

impl GarbageCollector for SledBackend {
    fn new_cycle_started(&mut self) -> Result<(), StorageBackendError> {
        Ok(())
    }

    fn mark_reused(
        &mut self,
        _reused_keys: std::collections::HashSet<EntryHash>,
    ) -> Result<(), StorageBackendError> {
        Ok(())
    }
}

impl KeyValueStoreBackend<MerkleStorage> for SledBackend {
    fn retain(&self, predicate: &dyn Fn(&EntryHash) -> bool) -> Result<(), DBError> {
        let garbage_keys: Vec<_> = self
            .inner
            .iter()
            .filter_map(|i| match i {
                Err(_) => None,
                Ok((k, _)) => {
                    let mut buffer = [0_u8; 32];
                    k.to_vec().reader().read_exact(&mut buffer).unwrap();
                    if !predicate(&buffer) {
                        Some(buffer)
                    } else {
                        None
                    }
                }
            })
            .collect();

        for k in garbage_keys {
            self.delete(&k)?;
        }
        Ok(())
    }

    fn put(&self, key: &EntryHash, value: &ContextValue) -> Result<(), DBError> {
        self.inner.insert(&key.as_ref()[..], value.clone())?;
        Ok(())
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

    fn write_batch(&self, batch: Vec<(EntryHash, ContextValue)>) -> Result<(), DBError> {
        for (k, v) in batch {
            self.merge(&k, &v)?;
        }
        Ok(())
    }

    fn total_get_mem_usage(&self) -> Result<usize, DBError> {
        self.db
            .size_on_disk()
            .map(|size| size as usize)
            .map_err(|e| DBError::SledDBError { error: e })
    }

    fn is_persistent(&self) -> bool {
        true
    }
}
