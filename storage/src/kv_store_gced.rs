use std::thread;
use std::mem;
use std::collections::{HashSet, VecDeque};
use std::time::{Instant, Duration};
use std::sync::{mpsc, Arc, RwLock};
use std::ops::{Deref, DerefMut};

use blake2::digest::VariableOutput;
use blake2::VarBlake2b;

use crypto::hash::HashType;

use crate::kv_store::KVStoreError;
use crate::merkle_storage::{KVStore, Entry, EntryHash, ContextValue};

fn stores_get<T, S>(stores: &S, key: &EntryHash) -> Option<ContextValue>
where T: KVStore,
      S: Deref<Target = Vec<T>>,
{
    stores.iter().rev()
        .find_map(|store| store.get(key).unwrap_or(None))
}

fn stores_containing<T, S>(stores: &S, key: &EntryHash) -> Option<usize>
where T: KVStore,
      S: Deref<Target = Vec<T>>,
{
    stores.iter().enumerate()
        .find(|(_, store)| store.contains(key).unwrap_or(false))
        .map(|(index, _)| index)
        // .map(|(rev_offset, _)| stores.len() - rev_offset - 1)
}

fn stores_contains<T, S>(stores: &S, key: &EntryHash) -> bool
where T: KVStore,
      S: Deref<Target = Vec<T>>,
{
    stores.iter().rev()
        .find(|store| store.contains(key).unwrap_or(false))
        .is_some()
}

/// finds in which store key is stored and deletes it
fn stores_delete<T, S>(stores: &mut S, key: &EntryHash) -> Option<ContextValue>
where T: KVStore,
      S: DerefMut<Target = Vec<T>>,
{
    stores.iter_mut().rev()
        .find_map(|store| store.delete(key).unwrap_or(None))
}

/// Commands used by KVStoreGCed to interact with thread.
pub enum CmdMsg {
    StartNewCycle,
    Exit,
    MarkReused(EntryHash),
}

/// Garbage Collected Key Value Store
pub struct KVStoreGCed<T: KVStore> {
    cycle_count: usize,
    stores: Arc<RwLock<Vec<T>>>,
    current: T,
    thread: thread::JoinHandle<()>,
    msg: mpsc::Sender<CmdMsg>,
}

impl<T: 'static + KVStore + Default> KVStoreGCed<T> {
    pub fn new(cycle_count: usize) -> Self {
        assert!(cycle_count > 1, "cycle_count less than 2 for KVStoreGCed not supported");

        let (tx, rx) = mpsc::channel();
        let stores = Arc::new(RwLock::new(
            (0..(cycle_count - 1)).map(|_| Default::default()).collect()
        ));

        Self {
            cycle_count,
            stores: stores.clone(),
            thread: thread::spawn(move || kvstore_gc_thread_fn(stores, rx)),
            msg: tx,
            current: Default::default(),
        }
    }

    pub fn is_persisted(&self) -> bool { false }

    fn stores_get(&self, key: &EntryHash) -> Option<ContextValue> {
        stores_get(&self.stores.read().unwrap(), key)
    }

    fn stores_contains(&self, key: &EntryHash) -> bool {
        stores_contains(&self.stores.read().unwrap(), key)
    }

    pub fn get(&self, key: &EntryHash) -> Option<ContextValue> {
        self.current.get(key)
            .unwrap_or(None)
            .or_else(|| self.stores_get(key))
    }

    pub fn contains(&self, key: &EntryHash) -> bool {
        self.current.contains(key).unwrap_or(false)
            || self.stores_contains(key)
    }

    pub fn put(
        &mut self,
        key: EntryHash,
        value: ContextValue,
    ) -> Result<(), KVStoreError> {
        self.current.put(key, value)
    }

    pub fn mark_reused(&mut self, key: EntryHash) {
        let _ = self.msg.send(CmdMsg::MarkReused(key));
    }

    pub fn start_new_cycle(&mut self) {
        let mut stores = self.stores.write().unwrap();
        stores.push(mem::take(&mut self.current));
        let _ = self.msg.send(CmdMsg::StartNewCycle);
    }

    pub fn wait_for_gc_finish(&self) {
        while self.stores.read().unwrap().len() >= self.cycle_count {
            thread::sleep(Duration::from_millis(2));
        }
    }
}

fn kvstore_gc_thread_fn<T: KVStore>(
    stores: Arc<RwLock<Vec<T>>>,
    rx: mpsc::Receiver<CmdMsg>,
) {
    let len = stores.read().unwrap().len();
    let mut reused_keys = vec![HashSet::new(); len];
    let mut todo_keys: VecDeque<EntryHash> = VecDeque::new();
    let mut received_exit_msg = false;
    let mut reused_count = 0;
    loop {
        let wait_for_events = reused_keys.len() == len && !received_exit_msg;

        let msg = if wait_for_events {
            match rx.recv() {
                Ok(value) => Some(value),
                Err(_) => {
                    println!("MerkleStorage GC thread shut down! reason: mpsc::Sender dropped.");
                    return;
                },
            }
        } else {
            match rx.try_recv() {
                Ok(value) => Some(value),
                Err(_) => None,
            }
        };

        match msg {
            Some(CmdMsg::StartNewCycle) => {
                reused_keys.push(Default::default());
                println!("reused_keys vec count: {}", reused_keys.len());
            }
            Some(CmdMsg::Exit) => {
                received_exit_msg = true;
            }
            Some(CmdMsg::MarkReused(key)) => {
                // TODO: refactor so that `mark_reused` isn't called
                // unless key was in MerkleStorage::db
                if let Some(index) = stores_containing(&stores.read().unwrap(), &key) {
                    if index < reused_keys.len() {
                        reused_keys[index].insert(key);
                    }
                }
            }
            None => {
                if received_exit_msg && todo_keys.len() == 0 && reused_keys.len() == len {
                    println!("MerkleStorage GC thread shut down! reason: received exit message.");
                    return;
                }
            }
        }

        if reused_keys.len() > len && reused_keys[0].len() > 0 {
            todo_keys.extend(mem::take(&mut reused_keys[0]).into_iter());
        }

        if todo_keys.len() > 0 {
            reused_count += todo_keys.len().min(16);
            let keys: Vec<_> = todo_keys.drain(..(todo_keys.len().min(16))).collect();
            let mut stores = stores.write().unwrap();
            for key in keys.into_iter() {
                let entry_bytes = stores_get(&stores, &key).unwrap();
                let entry: Entry = bincode::deserialize(&entry_bytes).unwrap();
                // let entry_bytes = match stores_get(&stores, &key) {
                //     Some(value) => value,
                //     None => continue,
                // };
                // let entry: Entry = match bincode::deserialize(&entry_bytes) {
                //     Ok(value) => value,
                //     Err(_) => continue
                // };
                stores.last_mut().unwrap().put(key.clone(), entry_bytes).unwrap();

                match entry {
                    Entry::Blob(_) => {}
                    Entry::Tree(tree) => {
                        let children = tree.into_iter()
                            .map(|(_, node)| node.entry_hash);
                        todo_keys.extend(children);
                    }
                    Entry::Commit(commit) => {
                        todo_keys.push_back(commit.root_hash);
                    }
                }
            }
        }

        if reused_keys.len() > len && reused_keys[0].len() == 0 && todo_keys.len() == 0 {
            reused_keys.drain(..1);
            let mut stores = stores.write().unwrap();
            println!("GC deleted: {} entries", stores[0].len().unwrap());
            println!("GC reused: {} entries", reused_count);
            reused_count = 0;
            stores.drain(..1);
            // stores.write().unwrap().drain(..1);
        }
    }

    println!("MerkleStorage GC thread shut down!");
}

// when key is reused

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem;
    use std::sync::mpsc;
    use crate::in_memory;

    fn empty_kvstore_gced(cycle_count: usize) -> KVStoreGCed<in_memory::KVStore<EntryHash, ContextValue>> {
        KVStoreGCed::new(cycle_count)
    }

    fn entry_hash(key: &[u8]) -> EntryHash {
        assert!(key.len() < 32);
        let mut result = [0u8; 32];

        for (index, value) in key.iter().enumerate() {
            result[index] = *value;
        }

        result
    }

    fn blob(value: Vec<u8>) -> Entry {
        Entry::Blob(value)
    }

    fn get<T: 'static + KVStore + Default>(store: &KVStoreGCed<T>, key: &[u8]) -> Option<Entry> {
        store.get(&entry_hash(key))
            .map(|x| bincode::deserialize(&x[..]).unwrap())
    }

    fn put<T: 'static + KVStore + Default>(store: &mut KVStoreGCed<T>, key: &[u8], value: Entry) {
        store.put(entry_hash(key), bincode::serialize(&value).unwrap()).unwrap();
    }

    fn mark_reused<T: 'static + KVStore + Default>(store: &mut KVStoreGCed<T>, key: &[u8]) {
        store.mark_reused(entry_hash(key));
    }

    #[test]
    fn test_key_reused_exists() {
        let store = &mut empty_kvstore_gced(3);

        put(store, &[1], blob(vec![1]));
        put(store, &[2], blob(vec![2]));
        store.start_new_cycle();
        put(store, &[3], blob(vec![3]));
        store.start_new_cycle();
        put(store, &[4], blob(vec![4]));
        mark_reused(store, &[1]);
        store.start_new_cycle();

        store.wait_for_gc_finish();

        assert_eq!(get(store, &[1]), Some(blob(vec![1])));
        assert_eq!(get(store, &[2]), None);
        assert_eq!(get(store, &[3]), Some(blob(vec![3])));
        assert_eq!(get(store, &[4]), Some(blob(vec![4])));
    }
}
