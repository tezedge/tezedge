// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::{HashSet, VecDeque};

use crate::merkle_storage::{ContextValue, EntryHash};
use crate::persistent::database::{DBError, KeyValueStoreBackend};
use crate::storage_backend::{GarbageCollector, StorageBackendError};
use crate::MerkleStorage;

/// Garbage Collected Key Value Store
pub struct MarkSweepGCed<T: KeyValueStoreBackend<MerkleStorage>> {
    store: T,
    cycles_limit: usize,
    cycles: VecDeque<HashSet<EntryHash>>,
}

impl<T: 'static + KeyValueStoreBackend<MerkleStorage> + Default> MarkSweepGCed<T> {
    pub fn new(cycle_count: usize) -> Self {
        let mut cycles = VecDeque::new();

        for _ in 0..cycle_count + 1 {
            //one extra buffer "current"
            cycles.push_back(HashSet::new());
        }

        Self {
            store: Default::default(),
            cycles_limit: cycle_count + 1,
            cycles,
        }
    }

    fn mark_reused(&mut self, reused_keys: HashSet<EntryHash>) -> Result<(), StorageBackendError> {
        if let Some(cycle) = self.cycles.back_mut() {
            cycle.extend(reused_keys);
        }
        Ok(())
    }

    pub fn new_cycle_started(&mut self) -> Result<(), StorageBackendError> {
        self.cycles.push_back(HashSet::new());

        while self.cycles.len() > self.cycles_limit {
            let _ = self.cycles.pop_front();
        }

        let mut entries_in_use = HashSet::new();
        for entries in self.cycles.iter() {
            entries_in_use.extend(entries);
        }

        self.sweep_entries(entries_in_use)?;

        Ok(())
    }

    fn sweep_entries(&mut self, todo: HashSet<EntryHash>) -> Result<(), StorageBackendError> {
        self.retain(&|x| todo.contains(x))?;
        Ok(())
    }
}

impl<T: 'static + KeyValueStoreBackend<MerkleStorage> + Default> GarbageCollector
    for MarkSweepGCed<T>
{
    fn new_cycle_started(&mut self) -> Result<(), StorageBackendError> {
        self.new_cycle_started()
    }

    fn mark_reused(&mut self, reused_keys: HashSet<EntryHash>) -> Result<(), StorageBackendError> {
        self.mark_reused(reused_keys)
    }
}

impl<T: 'static + KeyValueStoreBackend<MerkleStorage> + Default> KeyValueStoreBackend<MerkleStorage>
    for MarkSweepGCed<T>
{
    fn put(&self, key: &EntryHash, value: &ContextValue) -> Result<(), DBError> {
        println!("put {:?}", key);
        self.store.put(key, value)
    }

    fn delete(&self, key: &EntryHash) -> Result<(), DBError> {
        self.store.delete(key)
    }

    fn merge(&self, key: &EntryHash, value: &ContextValue) -> Result<(), DBError> {
        self.store.merge(key, value)
    }

    fn get(&self, key: &EntryHash) -> Result<Option<ContextValue>, DBError> {
        self.store.get(key)
    }

    fn contains(&self, key: &EntryHash) -> Result<bool, DBError> {
        self.store.contains(key)
    }

    fn write_batch(&self, batch: Vec<(EntryHash, ContextValue)>) -> Result<(), DBError> {
        Ok(self.store.write_batch(batch)?)
    }

    fn total_get_mem_usage(&self) -> Result<usize, DBError> {
        Ok(
            self.cycles
                .iter()
                .map(|set| set.len() * std::mem::size_of::<EntryHash>() as usize )
                .sum::<usize>()
            + self.store.total_get_mem_usage()?
        )
    }

    fn retain(&self, predicate: &dyn Fn(&EntryHash) -> bool) -> Result<(), DBError> {
        self.store.retain(predicate)
    }

    fn is_persistent(&self) -> bool {
        self.store.is_persistent()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::InMemoryBackend;
    use crate::merkle_storage::{hash_entry, Entry};
    use crate::merkle_storage::{Commit, Node, NodeKind};
    use im::ordmap;
    use std::collections::BTreeMap;

    #[test]
    fn test_mark_sweep_gc() {
        let value_1 = Entry::Blob(vec![1]);
        let value_1_hash = hash_entry(&value_1).unwrap();
        let tree_1 = Entry::Tree(ordmap! {
            "a".to_string() => Node{node_kind:NodeKind::Leaf, entry_hash: value_1_hash},
            "b".to_string() => Node{node_kind:NodeKind::Leaf, entry_hash: value_1_hash},
            "c".to_string() => Node{node_kind:NodeKind::Leaf, entry_hash: value_1_hash},
            "d".to_string() => Node{node_kind:NodeKind::Leaf, entry_hash: value_1_hash}
        });
        let commit_1 = Entry::Commit(Commit {
            parent_commit_hash: None,
            root_hash: hash_entry(&tree_1).unwrap(),
            time: 0,
            author: "dev".to_string(),
            message: "first commit".to_string(),
        });

        let value_2 = Entry::Blob(vec![2]);
        let value_2_hash = hash_entry(&value_2).unwrap();
        let tree_2 = Entry::Tree(ordmap! {
            "a".to_string() => Node{node_kind:NodeKind::Leaf, entry_hash: value_2_hash},
            "b".to_string() => Node{node_kind:NodeKind::Leaf, entry_hash: value_2_hash},
            "c".to_string() => Node{node_kind:NodeKind::Leaf, entry_hash: value_2_hash},
            "d".to_string() => Node{node_kind:NodeKind::Leaf, entry_hash: value_2_hash}
        });
        let commit_2 = Entry::Commit(Commit {
            parent_commit_hash: None,
            root_hash: hash_entry(&tree_2).unwrap(),
            time: 0,
            author: "dev".to_string(),
            message: "second commit".to_string(),
        });

        let mut store = MarkSweepGCed::<InMemoryBackend>::new(1);
        store
            .put(
                &hash_entry(&value_1).unwrap(),
                &bincode::serialize(&value_1).unwrap(),
            )
            .unwrap();
        store
            .put(
                &hash_entry(&tree_1).unwrap(),
                &bincode::serialize(&tree_1).unwrap(),
            )
            .unwrap();
        store
            .put(
                &hash_entry(&commit_1).unwrap(),
                &bincode::serialize(&commit_1).unwrap(),
            )
            .unwrap();

        // check valuas are avaible before running GC
        assert!(store.get(&hash_entry(&value_1).unwrap()).is_ok());
        assert!(store.get(&hash_entry(&tree_1).unwrap()).is_ok());
        assert!(store.get(&hash_entry(&commit_1).unwrap()).is_ok());

        // run GC
        store
            .mark_reused(
                vec![
                    hash_entry(&value_1).unwrap(),
                    hash_entry(&tree_1).unwrap(),
                    hash_entry(&commit_1).unwrap(),
                ]
                .into_iter()
                .collect(),
            )
            .unwrap();
        store.new_cycle_started().unwrap();

        // store.gc(Some(hash_entry(&commit_1).unwrap())).unwrap();

        // check valuas are available after GC
        assert!(store.get(&hash_entry(&value_1).unwrap()).unwrap().is_some());
        assert!(store.get(&hash_entry(&tree_1).unwrap()).unwrap().is_some());
        println!("inserting commit {:?}", hash_entry(&commit_1));
        assert!(store
            .get(&hash_entry(&commit_1).unwrap())
            .unwrap()
            .is_some());

        store
            .put(
                &hash_entry(&value_2).unwrap(),
                &bincode::serialize(&value_2).unwrap(),
            )
            .unwrap();
        store
            .put(
                &hash_entry(&tree_2).unwrap(),
                &bincode::serialize(&tree_2).unwrap(),
            )
            .unwrap();
        println!("inserting commit {:?}", hash_entry(&commit_2));
        store
            .put(
                &hash_entry(&commit_2).unwrap(),
                &bincode::serialize(&commit_2).unwrap(),
            )
            .unwrap();

        // run GC
        store
            .mark_reused(
                vec![
                    hash_entry(&value_1).unwrap(),
                    hash_entry(&value_2).unwrap(),
                    hash_entry(&tree_2).unwrap(),
                    hash_entry(&commit_2).unwrap(),
                ]
                .into_iter()
                .collect(),
            )
            .unwrap();
        store.new_cycle_started().unwrap();

        // new hashes should be still in storage
        assert!(store.get(&hash_entry(&value_1).unwrap()).unwrap().is_some());
        assert!(store.get(&hash_entry(&value_2).unwrap()).unwrap().is_some());
        assert!(store.get(&hash_entry(&tree_2).unwrap()).unwrap().is_some());
        assert!(store
            .get(&hash_entry(&commit_2).unwrap())
            .unwrap()
            .is_some());

        // those should be cleaned up
        assert!(store.get(&hash_entry(&tree_1).unwrap()).unwrap().is_none());
        assert!(store
            .get(&hash_entry(&commit_1).unwrap())
            .unwrap()
            .is_none());
    }
}
