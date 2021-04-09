// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;
use std::collections::{HashSet, VecDeque};

use crypto::hash::HashType;

use crate::gc::{collect_hashes, fetch_entry_from_store, GarbageCollectionError, GarbageCollector};
use crate::hash::EntryHash;
use crate::persistent::database::DBError;
use crate::persistent::KeyValueStoreBackend;
use crate::working_tree::Entry;
use crate::{ContextKeyValueStoreSchema, ContextValue};

/// Garbage Collected Key Value Store
pub struct MarkSweepGCed<T: KeyValueStoreBackend<ContextKeyValueStoreSchema>> {
    // TODO: rework gc store must be passed as argument, and not to create default here
    store: T,
    cycles_limit: usize,
    cycles: VecDeque<HashSet<EntryHash>>,
    cache: HashMap<EntryHash, HashSet<EntryHash>>,
}

impl<T: 'static + KeyValueStoreBackend<ContextKeyValueStoreSchema> + Default> MarkSweepGCed<T> {
    pub fn new(cycle_count: usize) -> Self {
        let mut cycles = VecDeque::new();

        for _ in 0..cycle_count + 1 {
            //one extra buffer "current"
            cycles.push_back(HashSet::new());
        }

        Self {
            // TODO: zrusti default
            store: Default::default(),
            cycles_limit: cycle_count + 1,
            cycles,
            cache: HashMap::new(),
        }
    }

    fn mark_reused(&mut self, reused_keys: HashSet<EntryHash>) {
        if let Some(cycle) = self.cycles.back_mut() {
            cycle.extend(reused_keys);
        }
    }

    pub fn new_cycle_started(&mut self) -> Result<(), GarbageCollectionError> {
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

    fn sweep_entries(&mut self, todo: HashSet<EntryHash>) -> Result<(), GarbageCollectionError> {
        self.retain(&|x| todo.contains(x))?;
        Ok(())
    }

    fn store_entries_referenced_by_commit(
        &mut self,
        commit: EntryHash,
    ) -> Result<(), GarbageCollectionError> {
        let commit_entry = fetch_entry_from_store(&self.store, commit)?;

        match commit_entry {
            Entry::Commit { .. } => {
                let mut entries = HashSet::new();
                collect_hashes(&commit_entry, &mut entries, &mut self.cache, &self.store)?;

                // remove keys non used in current block
                self.cache.retain(|k, _| entries.contains(k));
                self.mark_reused(entries.into_iter().collect());
                Ok(())
            }
            _ => Err(GarbageCollectionError::GarbageCollectorError {
                error: format!(
                    "{} is not a commit",
                    HashType::ContextHash.hash_to_b58check(&commit)?
                ),
            }),
        }
    }
}

impl<T: 'static + KeyValueStoreBackend<ContextKeyValueStoreSchema> + Default> GarbageCollector
    for MarkSweepGCed<T>
{
    fn new_cycle_started(&mut self) -> Result<(), GarbageCollectionError> {
        self.new_cycle_started()
    }

    fn block_applied(&mut self, commit: EntryHash) -> Result<(), GarbageCollectionError> {
        self.store_entries_referenced_by_commit(commit)
    }
}

impl<T: 'static + KeyValueStoreBackend<ContextKeyValueStoreSchema> + Default>
    KeyValueStoreBackend<ContextKeyValueStoreSchema> for MarkSweepGCed<T>
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
            + self.store.total_get_mem_usage()?
            + self
                .cache
                .iter()
                .map(|(_, v)| {
                    std::mem::size_of::<EntryHash>()
                        + v.len() * std::mem::size_of::<EntryHash>() as usize
                })
                .sum::<usize>())
    }

    fn retain(&self, predicate: &dyn Fn(&EntryHash) -> bool) -> Result<(), DBError> {
        self.store.retain(predicate)
    }
}

#[cfg(test)]
mod tests {
    use crate::hash::hash_entry;
    use crate::kv_store::in_memory_backend::InMemoryBackend;
    use crate::working_tree::Entry;

    use super::*;

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
        store.mark_reused(
            vec![hash_entry(&value_1).unwrap()]
                .into_iter()
                .collect::<HashSet<EntryHash>>(),
        );
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
        store.mark_reused(
            vec![hash_entry(&value_2).unwrap()]
                .into_iter()
                .collect::<HashSet<EntryHash>>(),
        );
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
        store.mark_reused(
            vec![hash_entry(&value_3).unwrap()]
                .into_iter()
                .collect::<HashSet<EntryHash>>(),
        );
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
        store.mark_reused(
            vec![hash_entry(&value_4).unwrap()]
                .into_iter()
                .collect::<HashSet<EntryHash>>(),
        );
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
        store.mark_reused(
            vec![hash_entry(&value_5).unwrap()]
                .into_iter()
                .collect::<HashSet<EntryHash>>(),
        );
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
        store.mark_reused(
            vec![hash_entry(&value_6).unwrap()]
                .into_iter()
                .collect::<HashSet<EntryHash>>(),
        );
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
        store.mark_reused(
            vec![hash_entry(&value_7).unwrap()]
                .into_iter()
                .collect::<HashSet<EntryHash>>(),
        );
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
        store.mark_reused(
            vec![hash_entry(&value_8).unwrap()]
                .into_iter()
                .collect::<HashSet<EntryHash>>(),
        );
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
