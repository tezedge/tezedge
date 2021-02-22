
use std::collections::{HashSet, VecDeque, HashMap};

use crate::persistent::database::RocksDBStats;
use crate::merkle_storage::{hash_entry, ContextValue, Entry, EntryHash};
use crate::storage_backend::{
    StorageBackend , StorageBackendError, StorageBackendStats,
};
/// Garbage Collected Key Value Store
pub struct MarkSweepGCed<T: StorageBackend> {
    store: T,
    cycles_limit: usize,
    blocks_per_cycle: usize,
    commits: VecDeque<EntryHash>,
    marked: HashMap<EntryHash, HashSet<EntryHash>>
}

impl<T: 'static + StorageBackend + Default> MarkSweepGCed<T> {
    pub fn new(cycle_count: usize, cycle_size: usize) -> Self {
        Self {
            store: Default::default(),
            cycles_limit: cycle_count, 
            blocks_per_cycle: cycle_size, 
            commits: VecDeque::new(),
            marked: HashMap::new()
        }
    }

    fn get_entry(&self, key: &EntryHash) -> Result<Option<Entry>, StorageBackendError> {
        match self.store.get(key)? {
            None => Ok(None),
            Some(entry_bytes) => Ok(Some(bincode::deserialize(&entry_bytes)?)),
        }
    }

    pub fn gc(&mut self, last_commit_hash: Option<EntryHash>) -> Result<(), StorageBackendError> {
        if let Some(hash) = last_commit_hash {
            let mut new_entries_in_use = HashSet::new();
            self.mark_entries(&mut new_entries_in_use, hash)?;
            self.commits.push_back(hash);
            self.marked.insert(hash, new_entries_in_use);

            while self.commits.len() > self.cycles_limit*self.blocks_per_cycle{
                if let Some(hash) = self.commits.pop_front(){
                    println!("removing commit {:?},", &hash);
                    self.marked.remove(&hash);
                }
            }

            let mut entries_in_use = HashSet::new();
            //collect all the used entries
            for (_,v) in self.marked.iter(){
                entries_in_use.extend(v.clone());
            }
            //sweep all unused entries
            self.sweep_entries(entries_in_use)?;

        }
        Ok(())
    }

    fn mark_entries(&self, todo: &mut HashSet<EntryHash>, commit_hash: EntryHash) -> Result<(), StorageBackendError>{
        if let Ok(Some(entry)) = self.get_entry(&commit_hash) {
            self.mark_entries_recursively(&entry, todo)?;
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
                for (_, child_node) in tree.iter() {
                    if let Ok(Some(entry)) = self.get_entry(&child_node.entry_hash) {
                        self.mark_entries_recursively(&entry, todo)?;
                    };
                }
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

    fn total_get_mem_usage(&self) -> Result<usize,StorageBackendError>{
        self.store.total_get_mem_usage()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::TryFrom;
    use crate::MerkleStorage;
    use crate::backend::InMemoryBackend;
    use crate::merkle_storage::{Node,Commit,NodeKind};
    use std::collections::BTreeMap;

    fn create_storage(cycle_count: usize, cycle_size: usize) -> Box<dyn StorageBackend + Sync + Send>{
        Box::new(MarkSweepGCed::<InMemoryBackend>::new(cycle_count, cycle_size))
    }

    macro_rules! map(
    { $($key:expr => $value:expr),+ } => {
        {
            let mut m = BTreeMap::new();
            $(
                m.insert($key, $value);
            )+
            m
        }
     };
    );

    #[test]
    fn test_mark_sweep_gc() {
        let value_1 = Entry::Blob(vec![1]);
        let value_1_hash = hash_entry(&value_1).unwrap();
        let tree_1 = Entry::Tree(map!{
            "a".to_string() => Node{node_kind:NodeKind::Leaf, entry_hash: value_1_hash},
            "b".to_string() => Node{node_kind:NodeKind::Leaf, entry_hash: value_1_hash},
            "c".to_string() => Node{node_kind:NodeKind::Leaf, entry_hash: value_1_hash},
            "d".to_string() => Node{node_kind:NodeKind::Leaf, entry_hash: value_1_hash}
        });
        let commit_1 = Entry::Commit(Commit{
            parent_commit_hash: None,
            root_hash: hash_entry(&tree_1).unwrap(),
            time: 0,
            author: "dev".to_string(),
            message: "first commit".to_string(),
        });

        let value_2 = Entry::Blob(vec![2]);
        let value_2_hash = hash_entry(&value_2).unwrap();
        let tree_2 = Entry::Tree(map!{
            "a".to_string() => Node{node_kind:NodeKind::Leaf, entry_hash: value_2_hash},
            "b".to_string() => Node{node_kind:NodeKind::Leaf, entry_hash: value_2_hash},
            "c".to_string() => Node{node_kind:NodeKind::Leaf, entry_hash: value_2_hash},
            "d".to_string() => Node{node_kind:NodeKind::Leaf, entry_hash: value_2_hash}
        });
        let commit_2 = Entry::Commit(Commit{
            parent_commit_hash: None,
            root_hash: hash_entry(&tree_2).unwrap(),
            time: 0,
            author: "dev".to_string(),
            message: "second commit".to_string(),
        });

        let mut store = MarkSweepGCed::<InMemoryBackend>::new(1, 1);
        store.put(&hash_entry(&value_1).unwrap(), bincode::serialize(&value_1).unwrap()).unwrap();
        store.put(&hash_entry(&tree_1).unwrap(), bincode::serialize(&tree_1).unwrap()).unwrap();
        store.put(&hash_entry(&commit_1).unwrap(), bincode::serialize(&commit_1).unwrap()).unwrap();

        // check valuas are avaible before running GC
        assert!(store.get(&hash_entry(&value_1).unwrap()).is_ok());
        assert!(store.get(&hash_entry(&tree_1).unwrap()).is_ok());
        assert!(store.get(&hash_entry(&commit_1).unwrap()).is_ok());

        // run GC
        store.start_new_cycle(Some(hash_entry(&commit_1).unwrap()));

        // check valuas are available after GC
        assert!(store.get(&hash_entry(&value_1).unwrap()).unwrap().is_some());
        assert!(store.get(&hash_entry(&tree_1).unwrap()).unwrap().is_some());
        println!("inserting commit {:?}", hash_entry(&commit_1));
        assert!(store.get(&hash_entry(&commit_1).unwrap()).unwrap().is_some());

        store.put(&hash_entry(&value_2).unwrap(), bincode::serialize(&value_2).unwrap()).unwrap();
        store.put(&hash_entry(&tree_2).unwrap(), bincode::serialize(&tree_2).unwrap()).unwrap();
        println!("inserting commit {:?}", hash_entry(&commit_2));
        store.put(&hash_entry(&commit_2).unwrap(), bincode::serialize(&commit_2).unwrap()).unwrap();

        // run GC
        store.start_new_cycle(Some(hash_entry(&commit_2).unwrap()));

        // new hashes should be still in storage
        assert!(store.get(&hash_entry(&value_2).unwrap()).unwrap().is_some());
        assert!(store.get(&hash_entry(&tree_2).unwrap()).unwrap().is_some());
        assert!(store.get(&hash_entry(&commit_2).unwrap()).unwrap().is_some());

        // those should be cleaned up
        assert!(store.get(&hash_entry(&value_1).unwrap()).unwrap().is_none());
        assert!(store.get(&hash_entry(&tree_1).unwrap()).unwrap().is_none());
        assert!(store.get(&hash_entry(&commit_1).unwrap()).unwrap().is_none());
    }
}
