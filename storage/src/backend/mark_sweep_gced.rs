use std::collections::HashSet;

use crate::merkle_storage::{hash_entry, ContextValue, Entry, EntryHash};
use crate::storage_backend::{
    StorageBackend as KVStore, StorageBackendError as KVStoreError,
    StorageBackendStats as KVStoreStats,
};

/// Garbage Collected Key Value Store
pub struct MarkSweepGCed<T: KVStore> {
    store: T,
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

    pub fn gc(&mut self, last_commit_hash: Option<EntryHash>) -> Result<(), KVStoreError> {
        if let Some(_) = &last_commit_hash {
            let mut todo = HashSet::new();
            self.mark_entries(&mut todo, last_commit_hash);
            self.sweep_entries(todo)?;
        }
        Ok(())
    }

    fn mark_entries(&self, todo: &mut HashSet<EntryHash>, last_commit_hash: Option<EntryHash>) {
        if let Some(entry_hash) = &last_commit_hash {
            if let Ok(Some(entry)) = self.get_entry(entry_hash) {
                self.mark_entries_recursively(&entry, todo);
            }
        }
    }

    fn sweep_entries(&mut self, todo: HashSet<EntryHash>) -> Result<(), KVStoreError> {
        self.retain(todo)
    }

    fn mark_entries_recursively(&self, entry: &Entry, todo: &mut HashSet<EntryHash>) {
        let hash = hash_entry(entry);
        match entry {
            Entry::Blob(_) => {
                todo.insert(hash);
            }
            Entry::Tree(tree) => {
                todo.insert(hash);
                tree.iter().for_each(|(_, child_node)| {
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

    fn put(&mut self, key: EntryHash, value: ContextValue) -> Result<bool, KVStoreError> {
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

    fn mark_reused(&mut self, _key: EntryHash) {}

    fn start_new_cycle(&mut self, last_commit_hash: Option<EntryHash>) {
        let _ = self.gc(last_commit_hash);
    }

    fn wait_for_gc_finish(&self) {}

    fn get_stats(&self) -> Vec<KVStoreStats> {
        self.store.get_stats()
    }
}
