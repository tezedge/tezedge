// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::{btree_map::Entry, BTreeMap};
use std::collections::HashSet;
use std::sync::RwLock;
use crypto::hash::HashType;
use crate::persistent::database::{KeyValueStoreWithSchemaIterator, SimpleKeyValueStoreWithSchema, DBError, RocksDBStats, IteratorMode, IteratorWithSchema};
use crate::MerkleStorage;
use std::ops::{DerefMut, AddAssign, SubAssign};

use crate::merkle_storage::{ContextValue, EntryHash};
use crate::storage_backend::{StorageBackend , StorageBackendError, StorageBackendStats};

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

impl SimpleKeyValueStoreWithSchema<MerkleStorage> for KVStore<EntryHash, ContextValue> {

    fn is_persistent(&self) -> bool{
        false
    }

    fn total_get_mem_usage(&self) -> Result<usize,DBError>{
        Ok(self.stats.read().unwrap().total_as_bytes())
    }

    /// put kv in map if key doesn't exist. If it does then return false.
    fn put(&self, key: &EntryHash, value: &ContextValue) -> Result<(), DBError> {
        match self
            .kv_map
            .write()
            .unwrap()
            .entry(*key) {
            Entry::Vacant(entry) => {
                self.stats.write().unwrap().deref_mut().add_assign(StorageBackendStats::from((key, value)));
                entry.insert(value.clone());
                Ok(())
            }
            _ => {
                Err(DBError::ValueExists{key: HashType::ContextHash.hash_to_b58check(key)?})
            }
        }
    }

    fn merge(&self, key: &EntryHash, value: &ContextValue) -> Result<(), DBError> {
        self.stats.write().unwrap().deref_mut().add_assign(StorageBackendStats::from((key, value)));
        if let Some(prev) = self.kv_map.write().unwrap().insert(*key, value.clone()){
            self.stats.write().unwrap().deref_mut().sub_assign(StorageBackendStats::from((key, value)));
        };
        Ok(())
    }

    fn delete(&self, key: &EntryHash) -> Result<(), DBError> {
        let removed_key = self.kv_map.write().unwrap().remove(key);

        if let Some(v) =  &removed_key{
            self.stats.write().unwrap().deref_mut().sub_assign(StorageBackendStats::from((key, v)));
        }

        Ok(())
    }

    fn get(&self, key: &EntryHash) -> Result<Option<ContextValue>, DBError> {
        Ok(self.kv_map.read().unwrap().get(key).cloned())
    }

    fn contains(&self, key: &EntryHash) -> Result<bool, DBError> {
        Ok(self.kv_map.read().unwrap().contains_key(key))
    }

    fn retain(&self, predicate: &dyn Fn(&EntryHash) -> bool) -> Result<(), DBError>{
        let garbage_keys: Vec<_> = self
            .kv_map
            .read()
            .unwrap()
            .iter()
            .filter_map(|(k, _)| {
                if !predicate(k) {
                    Some(*k)
                } else {
                    None
                }
            })
            .collect();

        for k in garbage_keys {
            self.delete(&k)?;
        }
        Ok(())
    }

    fn write_batch(&self, batch: Vec<(EntryHash, ContextValue)>) -> Result<(), DBError> {
        for (k,v) in batch{
            self.merge(&k,&v)?;
        }
        Ok(())
    }
}

// impl StorageBackend for KVStore<EntryHash, ContextValue> {
//     fn is_persisted(&self) -> bool {
//         false
//     }
//
//     fn total_get_mem_usage(&self) -> Result<usize,StorageBackendError>{
//         Ok(self.stats.total_as_bytes())
//     }
//
//     /// put kv in map if key doesn't exist. If it does then return false.
//     fn put(&mut self, key: &EntryHash, value: ContextValue) -> Result<bool, StorageBackendError> {
//         match self.kv_map.entry(*key) {
//             Entry::Vacant(entry) => {
//                 self.stats += StorageBackendStats::from((key, &value));
//                 entry.insert(value);
//                 Ok(true)
//             }
//             _ => Ok(false),
//         }
//     }
//
//     fn merge(&mut self, key: &EntryHash, value: ContextValue) -> Result<(), StorageBackendError> {
//         self.stats += StorageBackendStats::from((key, &value));
//         if let Some(prev) = self.kv_map.insert(*key, value){
//             self.stats -= StorageBackendStats::from((key, &prev));
//         };
//         Ok(())
//     }
//
//     fn delete(&mut self, key: &EntryHash) -> Result<Option<ContextValue>, StorageBackendError> {
//         let removed_key = self.kv_map.remove(key);
//
//         if let Some(v) =  &removed_key{
//             self.stats -= StorageBackendStats::from((key, v));
//         }
//
//         Ok(removed_key)
//     }
//
//     fn get(&self, key: &EntryHash) -> Result<Option<ContextValue>, StorageBackendError> {
//         Ok(self.kv_map.get(key).cloned())
//     }
//
//     fn contains(&self, key: &EntryHash) -> Result<bool, StorageBackendError> {
//         Ok(self.kv_map.contains_key(key))
//     }
// }

pub type BTreeMapBackend = KVStore<EntryHash, ContextValue>;
