// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::{btree_map::Entry, BTreeMap};
use std::sync::RwLock;
use crypto::hash::HashType;
use crate::persistent::database::{KeyValueStoreBackend, DBError};
use crate::MerkleStorage;
use std::ops::{DerefMut, AddAssign, SubAssign};

use crate::merkle_storage::{ContextValue, EntryHash};
use crate::storage_backend::StorageBackendStats;
use crate::storage_backend::size_of_vec;

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

impl KeyValueStoreBackend<MerkleStorage> for KVStore<EntryHash, ContextValue> {

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
            Entry::Occupied(mut entry) => {
                self.stats.write().unwrap().deref_mut().add_assign(StorageBackendStats::from((key, value)));
                self.stats.write().unwrap().deref_mut().sub_assign(StorageBackendStats::from((entry.key(), entry.get())));
                entry.insert(value.clone());
                Ok(())
            }
        }
    }

    fn merge(&self, key: &EntryHash, value: &ContextValue) -> Result<(), DBError> {
        self.stats.write().unwrap().deref_mut().add_assign(StorageBackendStats::from((key, value)));
        if let Some(prev) = self.kv_map.write().unwrap().insert(*key, value.clone()){
            self.stats.write().unwrap().deref_mut().sub_assign(StorageBackendStats::from((key, &prev)));
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

pub type BTreeMapBackend = KVStore<EntryHash, ContextValue>;


#[cfg(test)]
#[allow(unused_must_use)]
mod tests {
    use std::convert::TryFrom;
    use crate::merkle_storage::Entry;

    use super::*;

    fn blob(value: Vec<u8>) -> Entry {
        Entry::Blob(value)
    }

    fn entry_hash(key: &[u8]) -> EntryHash {
        assert!(key.len() < 32);
        let bytes: Vec<u8> = key
            .iter()
            .chain(std::iter::repeat(&0u8))
            .take(32)
            .cloned()
            .collect();

        EntryHash::try_from(bytes).unwrap()
    }

    fn blob_serialized(value: Vec<u8>) -> Vec<u8> {
        bincode::serialize(&blob(value)).unwrap()
    }

    #[test]
    fn test_put_get() {
        let kv1 = (entry_hash(&[1]), blob_serialized(vec![1]));
        let storage = BTreeMapBackend::default();
        storage.put(&kv1.0, &kv1.1).unwrap();
        let value = storage.get(&kv1.0).unwrap().unwrap();
        assert_eq!(value, kv1.1);
    }

    #[test]
    fn test_put_twice_fail() {
        let kv1 = (entry_hash(&[1]), blob_serialized(vec![1]));
        let storage = BTreeMapBackend::default();
        storage.put(&kv1.0, &kv1.1).unwrap();
        assert!(storage.put(&kv1.0, &kv1.1).is_err());
    }

    #[test]
    fn test_put_merge_get() {
        let key = entry_hash(&[1]);
        let val1 = blob_serialized(vec![1]);
        let val2 = blob_serialized(vec![2]);

        let storage = BTreeMapBackend::default();

        storage.put(&key, &val1).unwrap();
        storage.merge(&key, &val2).unwrap();

        let value = storage.get(&key).unwrap().unwrap();
        assert_eq!(value, val2);
    }

    #[test]
    fn test_put_delete_get() {
        let key = entry_hash(&[1]);
        let val1 = blob_serialized(vec![1]);

        let storage = BTreeMapBackend::default();

        storage.put(&key, &val1).unwrap();
        storage.delete(&key).unwrap();

        assert!(storage.get(&key).unwrap().is_none());
    }

    #[test]
    fn test_contains() {
        let key = entry_hash(&[1]);
        let val1 = blob_serialized(vec![1]);

        let storage = BTreeMapBackend::default();

        assert!(!storage.contains(&key).unwrap());
        storage.put(&key, &val1).unwrap();
        assert!(storage.contains(&key).unwrap());
    }

    #[test]
    fn test_delete_non_existing_key() {
        let key = entry_hash(&[1]);
        let storage = BTreeMapBackend::default();
        assert!(storage.delete(&key).is_ok());
    }

    #[test]
    fn test_try_delete_non_existing_key() {
        let key = entry_hash(&[1]);
        let storage = BTreeMapBackend::default();
        assert!(storage.try_delete(&key).unwrap().is_none());
    }

    #[test]
    fn test_try_delete_existing_key() {
        let key = entry_hash(&[1]);
        let val1 = blob_serialized(vec![1]);
        let storage = BTreeMapBackend::default();
        storage.put(&key, &val1).unwrap();
        assert_eq!(val1, storage.try_delete(&key).unwrap().unwrap());
    }

    #[test]
    fn test_write_batch() {
        let batch = vec![
            (entry_hash(&[1]), blob_serialized(vec![11])),
            (entry_hash(&[2]), blob_serialized(vec![22])),
            (entry_hash(&[3]), blob_serialized(vec![33])),
        ];
        let storage = BTreeMapBackend::default();
        storage.write_batch(batch).unwrap();
        assert!(storage.contains(&entry_hash(&[1])).unwrap());
        assert!(storage.contains(&entry_hash(&[2])).unwrap());
        assert!(storage.contains(&entry_hash(&[3])).unwrap());

        assert_eq!(blob_serialized(vec![11]), storage.get(&entry_hash(&[1])).unwrap().unwrap());
        assert_eq!(blob_serialized(vec![22]), storage.get(&entry_hash(&[2])).unwrap().unwrap());
        assert_eq!(blob_serialized(vec![33]), storage.get(&entry_hash(&[3])).unwrap().unwrap());
    }

    #[test]
    fn test_memory_consumption() {
        let entry1 = entry_hash(&[1]);
        let value1 = blob_serialized(vec![1,2,3,3,5]);
        let entry2 = entry_hash(&[2]);
        let value2 = blob_serialized(vec![11,22,33]);

        let storage = BTreeMapBackend::default();
        assert_eq!(0, storage.total_get_mem_usage().unwrap());

        // insert first entry
        storage.put(&entry1, &value1).unwrap();
        assert_eq!(std::mem::size_of::<EntryHash>() + size_of_vec(&value1), storage.total_get_mem_usage().unwrap());
        
        // change value under key
        storage.merge(&entry1, &value2).unwrap();
        assert_eq!(std::mem::size_of::<EntryHash>() + size_of_vec(&value2), storage.total_get_mem_usage().unwrap());

        storage.put(&entry2, &value2).unwrap();
        assert_eq!(
            2*std::mem::size_of::<EntryHash>()
            + size_of_vec(&value2)
            + size_of_vec(&value2),
            storage.total_get_mem_usage().unwrap());

        //remove first entry
        storage.delete(&entry1).unwrap();
        assert_eq!(
            std::mem::size_of::<EntryHash>()
            + size_of_vec(&value2),
            storage.total_get_mem_usage().unwrap());

        //remove second entry
        storage.delete(&entry2).unwrap();
        assert_eq!(0, storage.total_get_mem_usage().unwrap());
    }
}
