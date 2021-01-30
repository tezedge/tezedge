use std::thread;
use std::collections::{HashSet};

use crate::merkle_storage::{Entry, EntryHash, ContextValue, hash_entry};
use crate::storage_backend::{
    StorageBackend as KVStore,
    StorageBackendError as KVStoreError,
    StorageBackendStats as KVStoreStats,
    size_of_vec,
};

/// Garbage Collected Key Value Store
pub struct MarkSweepGCed<T: KVStore> {
    store: T
}

impl<T: 'static + KVStore + Default> MarkSweepGCed<T> {
    pub fn new(_cycle_count: usize) -> Self {
        Self {
          store: Default::default(),
        }
    }

    fn get_entry(&self, key: &EntryHash) -> Result<Option<Entry>, KVStoreError> {
        match self.store.get(key)? {
            None => Ok(None),
            Some(entry_bytes) => Ok(Some(bincode::deserialize(&entry_bytes)?)),
        }
    }

    pub fn gc(&mut self, last_commit_hash: Option<EntryHash>) -> Result<(), KVStoreError>{
        if let Some(_) = &last_commit_hash {
            let mut todo = HashSet::new();
            self.mark_entries(&mut todo, last_commit_hash);
            self.sweep_entries(todo);
        }
        Ok(())
    }

    fn mark_entries(&self, todo : &mut HashSet<EntryHash>, last_commit_hash: Option<EntryHash>) {
        if let Some(entry_hash) = &last_commit_hash {
            if let Ok(Some(entry)) = self.get_entry(entry_hash) {
                self.mark_entries_recursively(&entry, todo);
            }
        }
    }

    fn sweep_entries(&mut self, todo: HashSet<EntryHash>)  -> Result<(), KVStoreError> {
        self.retain(todo);
        Ok(())
    }

    fn mark_entries_recursively(&self, entry: &Entry, todo: &mut HashSet<EntryHash>)  {
        if let Ok(hash) = hash_entry(entry) {
            match entry {
                Entry::Blob(_) => {
                    todo.insert(hash);
                }
                Entry::Tree(tree) => {
                    todo.insert(hash);
                    tree.iter().for_each(|(key, child_node)| {
                        match self.get_entry(&child_node.entry_hash) {
                            Ok(Some(entry)) => self.mark_entries_recursively(&entry, todo),
                            _ => {}
                        };
                    });
                }
                Entry::Commit(commit) => {
                    todo.insert(hash);
                    match self.get_entry(&commit.root_hash) {
                        Ok(Some(entry)) => self.mark_entries_recursively(&entry, todo),
                        _ => {}
                        Err(_) => {}
                    }
                }
            }
        }
    }
}


impl<T: 'static + KVStore + Default> KVStore for MarkSweepGCed<T> {
    fn is_persisted(&self) -> bool {
        self.store.is_persisted()
    }

    fn get(&self, key: &EntryHash) -> Result<Option<ContextValue>, KVStoreError> {
        self.store.get(key)
    }

    fn contains(&self, key: &EntryHash) -> Result<bool, KVStoreError> {
        self.store.contains(key)
    }

    fn put(
        &mut self,
        key: EntryHash,
        value: ContextValue,
    ) -> Result<bool, KVStoreError> {
        self.store.put(key, value)
    }

    fn merge(&mut self, key: EntryHash, value: ContextValue) -> Result<(), KVStoreError> {
        self.store.merge(key, value)
    }

    fn delete(&mut self, key: &EntryHash) -> Result<Option<ContextValue>, KVStoreError> {
        self.store.delete(key)
    }

    fn retain(&mut self, pred: HashSet<EntryHash>) -> Result<(), KVStoreError> {
        self.store.retain(pred)
    }

    fn mark_reused(&mut self, _key: EntryHash) { }

    fn start_new_cycle(&mut self, last_commit_hash: Option<EntryHash>) {
        self.gc(last_commit_hash);
    }

    fn wait_for_gc_finish(&self) { }

    fn get_stats(&self) -> Vec<KVStoreStats> {
        self.store.get_stats()
    }
}


// #[cfg(test)]
// mod tests {
//     use super::*;
//     use std::mem;
//     use crate::backend::BTreeMapBackend;

//     fn empty_kvstore_gced(cycle_count: usize) -> KVStoreGCed<BTreeMapBackend> {
//         KVStoreGCed::new(cycle_count)
//     }

//     fn entry_hash(key: &[u8]) -> EntryHash {
//         assert!(key.len() < 32);
//         let mut result = [0u8; 32];

//         for (index, value) in key.iter().enumerate() {
//             result[index] = *value;
//         }

//         result
//     }

//     fn blob(value: Vec<u8>) -> Entry {
//         Entry::Blob(value)
//     }

//     fn blob_serialized(value: Vec<u8>) -> Vec<u8> {
//         bincode::serialize(&blob(value)).unwrap()
//     }

//     fn get<T: 'static + KVStore + Default>(store: &KVStoreGCed<T>, key: &[u8]) -> Option<Entry> {
//         store.get(&entry_hash(key)).unwrap()
//             .map(|x| bincode::deserialize(&x[..]).unwrap())
//     }

//     fn put<T: 'static + KVStore + Default>(store: &mut KVStoreGCed<T>, key: &[u8], value: Entry) {
//         store.put(entry_hash(key), bincode::serialize(&value).unwrap()).unwrap();
//     }

//     fn mark_reused<T: 'static + KVStore + Default>(store: &mut KVStoreGCed<T>, key: &[u8]) {
//         store.mark_reused(entry_hash(key));
//     }

//     #[test]
//     fn test_stats() {
//         let store = &mut empty_kvstore_gced(3);

//         let kv1 = (entry_hash(&[1]), blob_serialized(vec![1]));
//         let kv2 = (entry_hash(&[2]), blob_serialized(vec![1, 2]));
//         let kv3 = (entry_hash(&[3]), blob_serialized(vec![1, 2, 3]));
//         let kv4 = (entry_hash(&[4]), blob_serialized(vec![1, 2, 3, 4]));

//         store.put(kv1.0.clone(), kv1.1.clone()).unwrap();
//         store.put(kv2.0.clone(), kv2.1.clone()).unwrap();
//         store.start_new_cycle();
//         store.put(kv3.0.clone(), kv3.1.clone()).unwrap();
//         store.start_new_cycle();
//         store.put(kv4.0.clone(), kv4.1.clone()).unwrap();
//         store.mark_reused(kv1.0.clone());

//         store.wait_for_gc_finish();

//         let stats: Vec<_> = store.get_stats().into_iter().rev().take(3).rev().collect();
//         assert_eq!(stats[0].key_bytes, 64);
//         assert_eq!(stats[0].value_bytes, size_of_vec(&kv1.1) + size_of_vec(&kv2.1));
//         assert_eq!(stats[0].reused_keys_bytes, 96);

//         assert_eq!(stats[1].key_bytes, 32);
//         assert_eq!(stats[1].value_bytes, size_of_vec(&kv3.1));
//         assert_eq!(stats[1].reused_keys_bytes, 0);

//         assert_eq!(stats[2].key_bytes, 32);
//         assert_eq!(stats[2].value_bytes, size_of_vec(&kv4.1));
//         assert_eq!(stats[2].reused_keys_bytes, 0);

//         assert_eq!(store.total_mem_usage_as_bytes(), vec![
//             4 * mem::size_of::<EntryHash>(),
//             96, // reused keys
//             size_of_vec(&kv1.1),
//             size_of_vec(&kv2.1),
//             size_of_vec(&kv3.1),
//             size_of_vec(&kv4.1),
//         ].iter().sum::<usize>());

//         store.start_new_cycle();
//         store.wait_for_gc_finish();

//         let stats = store.get_stats();
//         assert_eq!(stats[0].key_bytes, 32);
//         assert_eq!(stats[0].value_bytes, size_of_vec(&kv3.1));
//         assert_eq!(stats[0].reused_keys_bytes, 0);

//         assert_eq!(stats[1].key_bytes, 64);
//         assert_eq!(stats[1].value_bytes, size_of_vec(&kv1.1) + size_of_vec(&kv4.1));
//         assert_eq!(stats[1].reused_keys_bytes, 0);

//         assert_eq!(stats[2].key_bytes, 0);
//         assert_eq!(stats[2].value_bytes, 0);
//         assert_eq!(stats[2].reused_keys_bytes, 0);

//         assert_eq!(store.total_mem_usage_as_bytes(), vec![
//             3 * mem::size_of::<EntryHash>(),
//             size_of_vec(&kv1.1),
//             size_of_vec(&kv3.1),
//             size_of_vec(&kv4.1),
//         ].iter().sum::<usize>());
//     }
// }
