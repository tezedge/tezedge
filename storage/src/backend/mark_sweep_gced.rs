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
        Ok(self
            .cycles
            .iter()
            .map(|set| set.len() * std::mem::size_of::<EntryHash>() as usize)
            .sum::<usize>()
            + self.store.total_get_mem_usage()?)
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

    #[test]
    fn test_mark_sweep_gc() {
        let value_1 = Entry::Blob(vec![1]);
        let value_2 = Entry::Blob(vec![2]);
        let value_3 = Entry::Blob(vec![3]);
        let value_4 = Entry::Blob(vec![4]);
        let value_5 = Entry::Blob(vec![5]);
        let value_6 = Entry::Blob(vec![6]);
        let value_7 = Entry::Blob(vec![7]);
        let value_8 = Entry::Blob(vec![8]);

        let mut store = MarkSweepGCed::<InMemoryBackend>::new(4);
        // CYCLE 1
        store
            .put(
                &hash_entry(&value_1).unwrap(),
                &bincode::serialize(&value_1).unwrap(),
            )
            .unwrap();
        store
            .mark_reused(
                vec![hash_entry(&value_1).unwrap()]
                    .into_iter()
                    .collect::<HashSet<EntryHash>>(),
            )
            .unwrap();
        store.new_cycle_started().unwrap();
        assert!(!store
            .get(&hash_entry(&value_1).unwrap())
            .unwrap()
            .unwrap()
            .is_empty());

        // CYCLE 2
        store
            .put(
                &hash_entry(&value_2).unwrap(),
                &bincode::serialize(&value_2).unwrap(),
            )
            .unwrap();
        store
            .mark_reused(
                vec![hash_entry(&value_2).unwrap()]
                    .into_iter()
                    .collect::<HashSet<EntryHash>>(),
            )
            .unwrap();
        store.new_cycle_started().unwrap();
        assert!(!store
            .get(&hash_entry(&value_1).unwrap())
            .unwrap()
            .unwrap()
            .is_empty());
        assert!(!store
            .get(&hash_entry(&value_2).unwrap())
            .unwrap()
            .unwrap()
            .is_empty());

        // CYCLE 3
        store
            .put(
                &hash_entry(&value_3).unwrap(),
                &bincode::serialize(&value_3).unwrap(),
            )
            .unwrap();
        store
            .mark_reused(
                vec![hash_entry(&value_3).unwrap()]
                    .into_iter()
                    .collect::<HashSet<EntryHash>>(),
            )
            .unwrap();
        store.new_cycle_started().unwrap();
        assert!(!store
            .get(&hash_entry(&value_1).unwrap())
            .unwrap()
            .unwrap()
            .is_empty());
        assert!(!store
            .get(&hash_entry(&value_2).unwrap())
            .unwrap()
            .unwrap()
            .is_empty());
        assert!(!store
            .get(&hash_entry(&value_3).unwrap())
            .unwrap()
            .unwrap()
            .is_empty());

        // CYCLE 4
        store
            .put(
                &hash_entry(&value_4).unwrap(),
                &bincode::serialize(&value_4).unwrap(),
            )
            .unwrap();
        store
            .mark_reused(
                vec![hash_entry(&value_4).unwrap()]
                    .into_iter()
                    .collect::<HashSet<EntryHash>>(),
            )
            .unwrap();
        store.new_cycle_started().unwrap();
        assert!(!store
            .get(&hash_entry(&value_1).unwrap())
            .unwrap()
            .unwrap()
            .is_empty());
        assert!(!store
            .get(&hash_entry(&value_2).unwrap())
            .unwrap()
            .unwrap()
            .is_empty());
        assert!(!store
            .get(&hash_entry(&value_3).unwrap())
            .unwrap()
            .unwrap()
            .is_empty());
        assert!(!store
            .get(&hash_entry(&value_4).unwrap())
            .unwrap()
            .unwrap()
            .is_empty());

        // CYCLE 5
        store
            .put(
                &hash_entry(&value_5).unwrap(),
                &bincode::serialize(&value_5).unwrap(),
            )
            .unwrap();
        store
            .mark_reused(
                vec![hash_entry(&value_5).unwrap()]
                    .into_iter()
                    .collect::<HashSet<EntryHash>>(),
            )
            .unwrap();
        store.new_cycle_started().unwrap();
        assert!(!store
            .get(&hash_entry(&value_2).unwrap())
            .unwrap()
            .unwrap()
            .is_empty());
        assert!(!store
            .get(&hash_entry(&value_3).unwrap())
            .unwrap()
            .unwrap()
            .is_empty());
        assert!(!store
            .get(&hash_entry(&value_4).unwrap())
            .unwrap()
            .unwrap()
            .is_empty());
        assert!(!store
            .get(&hash_entry(&value_5).unwrap())
            .unwrap()
            .unwrap()
            .is_empty());

        // CYCLE 6
        store
            .put(
                &hash_entry(&value_6).unwrap(),
                &bincode::serialize(&value_6).unwrap(),
            )
            .unwrap();
        store
            .mark_reused(
                vec![hash_entry(&value_6).unwrap()]
                    .into_iter()
                    .collect::<HashSet<EntryHash>>(),
            )
            .unwrap();
        store.new_cycle_started().unwrap();
        assert!(!store
            .get(&hash_entry(&value_3).unwrap())
            .unwrap()
            .unwrap()
            .is_empty());
        assert!(!store
            .get(&hash_entry(&value_4).unwrap())
            .unwrap()
            .unwrap()
            .is_empty());
        assert!(!store
            .get(&hash_entry(&value_5).unwrap())
            .unwrap()
            .unwrap()
            .is_empty());
        assert!(!store
            .get(&hash_entry(&value_6).unwrap())
            .unwrap()
            .unwrap()
            .is_empty());

        // CYCLE 7
        store
            .put(
                &hash_entry(&value_7).unwrap(),
                &bincode::serialize(&value_7).unwrap(),
            )
            .unwrap();
        store
            .mark_reused(
                vec![hash_entry(&value_7).unwrap()]
                    .into_iter()
                    .collect::<HashSet<EntryHash>>(),
            )
            .unwrap();
        store.new_cycle_started().unwrap();
        assert!(!store
            .get(&hash_entry(&value_4).unwrap())
            .unwrap()
            .unwrap()
            .is_empty());
        assert!(!store
            .get(&hash_entry(&value_5).unwrap())
            .unwrap()
            .unwrap()
            .is_empty());
        assert!(!store
            .get(&hash_entry(&value_6).unwrap())
            .unwrap()
            .unwrap()
            .is_empty());
        assert!(!store
            .get(&hash_entry(&value_7).unwrap())
            .unwrap()
            .unwrap()
            .is_empty());

        // CYCLE 8
        store
            .put(
                &hash_entry(&value_8).unwrap(),
                &bincode::serialize(&value_8).unwrap(),
            )
            .unwrap();
        store
            .mark_reused(
                vec![hash_entry(&value_8).unwrap()]
                    .into_iter()
                    .collect::<HashSet<EntryHash>>(),
            )
            .unwrap();
        store.new_cycle_started().unwrap();
        assert!(!store
            .get(&hash_entry(&value_5).unwrap())
            .unwrap()
            .unwrap()
            .is_empty());
        assert!(!store
            .get(&hash_entry(&value_6).unwrap())
            .unwrap()
            .unwrap()
            .is_empty());
        assert!(!store
            .get(&hash_entry(&value_7).unwrap())
            .unwrap()
            .unwrap()
            .is_empty());
        assert!(!store
            .get(&hash_entry(&value_8).unwrap())
            .unwrap()
            .unwrap()
            .is_empty());
    }
}
