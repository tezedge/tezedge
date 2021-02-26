// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::persistent::database::{DBError, KeyValueStoreBackend};
use crate::MerkleStorage;
use std::collections::BTreeMap;
use std::ops::{AddAssign, DerefMut, SubAssign};
use std::sync::RwLock;

use crate::merkle_storage::{ContextValue, EntryHash};
use crate::storage_backend::{GarbageCollector, StorageBackendStats};

/// In Memory Key Value Store implemented with [BTreeMap](std::collections::BTreeMap)
#[derive(Debug)]
pub struct KVStore<K: Ord, V> {
    kv_map: RwLock<BTreeMap<K, V>>,
    stats: RwLock<StorageBackendStats>,
}

impl<K: Ord, V> Default for KVStore<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Ord, V> KVStore<K, V> {
    pub fn new() -> Self {
        Self {
            kv_map: RwLock::new(BTreeMap::new()),
            stats: Default::default(),
        }
    }
}

impl GarbageCollector for KVStore<EntryHash, ContextValue> {}

impl KeyValueStoreBackend<MerkleStorage> for KVStore<EntryHash, ContextValue> {
    fn is_persistent(&self) -> bool {
        false
    }

    fn total_get_mem_usage(&self) -> Result<usize, DBError> {
        Ok(self.stats.read()?.total_as_bytes())
    }

    /// put kv in map if key doesn't exist. If it does then return false.
    fn put(&self, key: &EntryHash, value: &ContextValue) -> Result<(), DBError> {
        let mut map = self.kv_map.write()?;

        match map.insert(*key, value.clone()) {
            Some(prev) => {
                self.stats
                    .write()?
                    .deref_mut()
                    .add_assign(StorageBackendStats::from((key, value)));
                self.stats
                    .write()?
                    .deref_mut()
                    .sub_assign(StorageBackendStats::from((key, &prev)));
                Ok(())
            }
            None => {
                self.stats
                    .write()?
                    .deref_mut()
                    .add_assign(StorageBackendStats::from((key, value)));
                Ok(())
            }
        }
    }

    fn merge(&self, key: &EntryHash, value: &ContextValue) -> Result<(), DBError> {
        self.stats
            .write()?
            .deref_mut()
            .add_assign(StorageBackendStats::from((key, value)));
        if let Some(prev) = self.kv_map.write()?.insert(*key, value.clone()) {
            self.stats
                .write()?
                .deref_mut()
                .sub_assign(StorageBackendStats::from((key, &prev)));
        };
        Ok(())
    }

    fn delete(&self, key: &EntryHash) -> Result<(), DBError> {
        let removed_key = self.kv_map.write()?.remove(key);

        if let Some(v) = &removed_key {
            self.stats
                .write()?
                .deref_mut()
                .sub_assign(StorageBackendStats::from((key, v)));
        }

        Ok(())
    }

    fn get(&self, key: &EntryHash) -> Result<Option<ContextValue>, DBError> {
        Ok(self.kv_map.read()?.get(key).cloned())
    }

    fn contains(&self, key: &EntryHash) -> Result<bool, DBError> {
        Ok(self.kv_map.read()?.contains_key(key))
    }

    fn retain(&self, predicate: &dyn Fn(&EntryHash) -> bool) -> Result<(), DBError> {
        let garbage_keys: Vec<_> = self
            .kv_map
            .read()?
            .iter()
            .filter_map(|(k, _)| if !predicate(k) { Some(*k) } else { None })
            .collect();

        for k in garbage_keys {
            self.delete(&k)?;
        }
        Ok(())
    }

    fn write_batch(&self, batch: Vec<(EntryHash, ContextValue)>) -> Result<(), DBError> {
        for (k, v) in batch {
            self.merge(&k, &v)?;
        }
        Ok(())
    }
}

pub type BTreeMapBackend = KVStore<EntryHash, ContextValue>;
