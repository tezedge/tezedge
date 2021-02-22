
use std::collections::HashSet;

use crate::persistent::database::RocksDBStats;
use crate::merkle_storage::{hash_entry, ContextValue, Entry, EntryHash};
use crate::storage_backend::{
    StorageBackend , StorageBackendError, StorageBackendStats,
};
/// Garbage Collected Key Value Store
pub struct MarkSweepGCed<T: StorageBackend> {
    store: T,
}

impl<T: 'static + StorageBackend + Default> MarkSweepGCed<T> {
    pub fn new(_cycle_count: usize) -> Self {
        Self {
            store: Default::default(),
        }
    }

    fn get_entry(&self, key: &EntryHash) -> Result<Option<Entry>, StorageBackendError> {
        match self.store.get(key)? {
            None => Ok(None),
            Some(entry_bytes) => Ok(Some(bincode::deserialize(&entry_bytes)?)),
        }
    }

    pub fn gc(&mut self, last_commit_hash: Option<EntryHash>) -> Result<(), StorageBackendError> {
        if let Some(_) = &last_commit_hash {
            let mut todo = HashSet::new();
            self.mark_entries(&mut todo, last_commit_hash);
            self.sweep_entries(todo)?;
        }
        Ok(())
    }

    fn mark_entries(&self, todo: &mut HashSet<EntryHash>, last_commit_hash: Option<EntryHash>) -> Result<(), StorageBackendError>{
        if let Some(entry_hash) = &last_commit_hash {
            if let Ok(Some(entry)) = self.get_entry(entry_hash) {
                self.mark_entries_recursively(&entry, todo)?;
            }
        }
        Ok(())
    }

    fn sweep_entries(&mut self, todo: HashSet<EntryHash>) -> Result<(), StorageBackendError> {
        self.retain(todo)
    }

    fn mark_entries_recursively(&self, entry: &Entry, todo: &mut HashSet<EntryHash>) -> Result<(), StorageBackendError>{
        let hash = hash_entry(entry)?;
        match entry {
            Entry::Blob(_) => {
                todo.insert(hash);
            }
            Entry::Tree(tree) => {
                todo.insert(hash);
                for (_,child_node) in tree.iter(){
                    if let Ok(Some(entry)) = self.get_entry(&child_node.entry_hash) {
                        self.mark_entries_recursively(&entry, todo)?;
                    };
                };
            }
            Entry::Commit(commit) => {
                todo.insert(hash);
                if let Ok(Some(entry)) = self.get_entry(&commit.root_hash) {
                    self.mark_entries_recursively(&entry, todo)?;
                }
            }
        };
        Ok(())
    }
}

impl<T: 'static + StorageBackend + Default> StorageBackend for MarkSweepGCed<T> {
    fn is_persisted(&self) -> bool {
        self.store.is_persisted()
    }

    fn get(&self, key: &EntryHash) -> Result<Option<ContextValue>, StorageBackendError> {
        self.store.get(key)
    }

    fn contains(&self, key: &EntryHash) -> Result<bool, StorageBackendError> {
        self.store.contains(key)
    }

    fn put(&mut self, key: &EntryHash, value: ContextValue) -> Result<bool, StorageBackendError> {
        self.store.put(key, value)
    }

    fn merge(&mut self, key: &EntryHash, value: ContextValue) -> Result<(), StorageBackendError> {
        self.store.merge(key, value)
    }

    fn delete(&mut self, key: &EntryHash) -> Result<Option<ContextValue>, StorageBackendError> {
        self.store.delete(key)
    }

    fn retain(&mut self, pred: HashSet<EntryHash>) -> Result<(), StorageBackendError> {
        self.store.retain(pred)
    }

    fn mark_reused(&mut self, _key: EntryHash) {}

    fn start_new_cycle(&mut self, last_commit_hash: Option<EntryHash>) {
        let _ = self.gc(last_commit_hash);
    }

    fn wait_for_gc_finish(&self) {}

    fn get_stats(&self) -> Vec<StorageBackendStats> {
        self.store.get_stats()
    }

    fn get_mem_use_stats(&self) -> Result<RocksDBStats, StorageBackendError> {
        //TODO TE-431 StorageBackent::get_mem_use_stats() should be implemented for all backends
        Ok(RocksDBStats {
            mem_table_total: 0,
            mem_table_unflushed: 0,
            mem_table_readers_total: 0,
            cache_total: 0,
        })
    }
}
