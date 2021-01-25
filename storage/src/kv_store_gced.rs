use std::thread;
use std::mem;
use std::collections::{HashSet};
use std::time::{Duration};
use std::sync::{mpsc, Arc, RwLock, Mutex};
use std::ops::{Deref, DerefMut};

use crypto::hash::HashType;
use serde::Serialize;

use crate::kv_store::KVStoreError;
use crate::merkle_storage::{KVStore, Entry, EntryHash, ContextValue};

// TODO: add assertions for EntryHash to make sure it is stack allocated.

fn size_of_vec<T>(v: &Vec<T>) -> usize {
    mem::size_of::<Vec<T>>() + mem::size_of::<T>() * v.capacity()
}

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
fn stores_delete<T, S>(stores: &mut S, key: &EntryHash) -> Option<(usize, ContextValue)>
where T: KVStore,
      S: DerefMut<Target = Vec<T>>,
{
    stores.iter_mut().enumerate()
        .find_map(|(index, store)| {
            store.delete(key)
                .unwrap_or(None)
                .map(|value| (index, value))
        })
}

/// Commands used by KVStoreGCed to interact with thread.
pub enum CmdMsg {
    StartNewCycle,
    Exit,
    MarkReused(EntryHash),
}

// TODO: measure reused keys size
#[derive(Debug, Default, Clone, Copy, Serialize)]
pub struct KVStoreStats {
    pub key_bytes: usize,
    pub value_bytes: usize,
    pub reused_keys_bytes: usize,
}

impl KVStoreStats {
    /// increases `reused_keys_bytes` based on `key`
    pub fn update_reused_keys(&mut self, list: &HashSet<EntryHash>) {
        self.reused_keys_bytes += list.capacity() * mem::size_of::<EntryHash>();
    }

    pub fn total_as_bytes(&self) -> usize {
        self.key_bytes + self.value_bytes + self.reused_keys_bytes
    }
}

impl<'a> std::ops::Add<&'a Self> for KVStoreStats {
    type Output = Self;

    fn add(self, other: &'a Self) -> Self::Output {
        Self {
            key_bytes: self.key_bytes + other.key_bytes,
            value_bytes: self.value_bytes + other.value_bytes,
            reused_keys_bytes: self.reused_keys_bytes + other.reused_keys_bytes,
        }
    }
}

impl std::ops::Add for KVStoreStats {
    type Output = Self;

    fn add(self, other: Self) -> Self::Output {
        self + &other
    }
}

impl<'a> std::ops::AddAssign<&'a Self> for KVStoreStats {
    fn add_assign(&mut self, other: &'a Self) {
        *self = *self + other;
    }
}

impl std::ops::AddAssign for KVStoreStats {
    fn add_assign(&mut self, other: Self) {
        *self = *self + other;
    }
}

impl<'a> std::ops::Sub<&'a Self> for KVStoreStats {
    type Output = Self;

    fn sub(self, other: &'a Self) -> Self::Output {
        Self {
            key_bytes: self.key_bytes - other.key_bytes,
            value_bytes: self.value_bytes - other.value_bytes,
            reused_keys_bytes: self.reused_keys_bytes - other.reused_keys_bytes,
        }
    }
}

impl std::ops::Sub for KVStoreStats {
    type Output = Self;

    fn sub(self, other: Self) -> Self::Output {
        self - &other
    }
}

impl<'a> std::ops::SubAssign<&'a Self> for KVStoreStats {
    fn sub_assign(&mut self, other: &'a Self) {
        *self = *self - other;
    }
}

impl std::ops::SubAssign for KVStoreStats {
    fn sub_assign(&mut self, other: Self) {
        *self = *self - other;
    }
}

impl<'a> std::iter::Sum<&'a KVStoreStats> for KVStoreStats {
    fn sum<I: Iterator<Item = &'a Self>>(iter: I) -> Self {
        iter.fold(KVStoreStats::default(), |acc, cur| {
            acc + cur
        })
    }
}

impl From<(&EntryHash, &ContextValue)> for KVStoreStats {
    fn from((_entry_hash, value): (&EntryHash, &ContextValue)) -> Self {
        KVStoreStats {
            key_bytes: mem::size_of::<EntryHash>(),
            value_bytes: size_of_vec(&value),
            reused_keys_bytes: 0,
        }
    }
}


/// Garbage Collected Key Value Store
pub struct KVStoreGCed<T: KVStore> {
    cycle_count: usize,
    stores: Arc<RwLock<Vec<T>>>,
    stores_stats: Arc<Mutex<Vec<KVStoreStats>>>,
    current: T,
    current_stats: KVStoreStats,
    thread: thread::JoinHandle<()>,
    // TODO: Mutex because it's required to be Sync. Better way?
    msg: Mutex<mpsc::Sender<CmdMsg>>,
}

impl<T: 'static + KVStore + Default> KVStoreGCed<T> {
    pub fn new(cycle_count: usize) -> Self {
        assert!(cycle_count > 1, "cycle_count less than 2 for KVStoreGCed not supported");

        let (tx, rx) = mpsc::channel();
        let stores = Arc::new(RwLock::new(
            (0..(cycle_count - 1)).map(|_| Default::default()).collect()
        ));
        let stores_stats = Arc::new(Mutex::new(
            vec![Default::default(); cycle_count - 1]
        ));

        Self {
            cycle_count,
            stores: stores.clone(),
            stores_stats: stores_stats.clone(),
            thread: thread::spawn(move || kvstore_gc_thread_fn(stores, stores_stats, rx)),
            msg: Mutex::new(tx),
            current: Default::default(),
            current_stats: Default::default(),
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
    ) -> Result<bool, KVStoreError> {
        let measurement = KVStoreStats::from((&key, &value));
        let was_added = self.current.put(key, value)?;

        if was_added {
            self.current_stats += measurement;
        }

        Ok(was_added)
    }

    pub fn mark_reused(&mut self, key: EntryHash) {
        let _ = self.msg.lock().unwrap().send(CmdMsg::MarkReused(key));
    }

    pub fn start_new_cycle(&mut self) {
        self.stores_stats.lock().unwrap().push(
            mem::take(&mut self.current_stats)
        );
        self.stores.write().unwrap().push(
            mem::take(&mut self.current)
        );
        let _ = self.msg.lock().unwrap().send(CmdMsg::StartNewCycle);
    }

    pub fn wait_for_gc_finish(&self) {
        while self.stores.read().unwrap().len() >= self.cycle_count {
            thread::sleep(Duration::from_millis(2));
        }
    }

    pub fn get_stats(&self) -> Vec<KVStoreStats> {
        self.stores_stats.lock().unwrap().iter()
            .chain(vec![&self.current_stats])
            .cloned()
            .collect()
    }

    pub fn get_total_stats(&self) -> KVStoreStats {
        self.get_stats().iter().sum()
    }

    pub fn total_mem_usage_as_bytes(&self) -> usize {
        self.get_total_stats().total_as_bytes()
    }
}

fn kvstore_gc_thread_fn<T: KVStore>(
    stores: Arc<RwLock<Vec<T>>>,
    stores_stats: Arc<Mutex<Vec<KVStoreStats>>>,
    rx: mpsc::Receiver<CmdMsg>,
) {
    let len = stores.read().unwrap().len();
    let mut reused_keys = vec![HashSet::new(); len];
    let mut todo_keys: Vec<EntryHash> = vec![];
    let mut received_exit_msg = false;

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
                        let keys = &mut reused_keys[index];
                        if keys.insert(key) {
                            stores_stats.lock().unwrap()[index].update_reused_keys(keys);
                        }
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
            let processed_len = todo_keys.len().min(64);

            let mut stats = stores_stats.lock().unwrap();
            let mut stores = stores.write().unwrap();
            for _ in 0..processed_len {
                let key = match todo_keys.pop() {
                    Some(key) => key,
                    None => break, // unreachable!
                };

                let (store_index, entry_bytes) = match stores_delete(&mut stores, &key) {
                    Some(result) => result,
                    None => continue,
                };
                let stat: KVStoreStats = (&key, &entry_bytes).into();
                stats[store_index] -= &stat;

                let entry: Entry = match bincode::deserialize(&entry_bytes) {
                    Ok(value) => value,
                    Err(err) => {
                        eprintln!("MerkleStorage GC: error while decerializing entry: {:?}", err);
                        continue;
                    }
                };

                if let Err(err) = stores.last_mut().unwrap().put(key.clone(), entry_bytes) {
                    eprintln!("MerkleStorage GC: error while adding entry to store: {:?}", err);
                } else {
                    *stats.last_mut().unwrap() += &stat;
                }

                match entry {
                    Entry::Blob(_) => {}
                    Entry::Tree(tree) => {
                        let children = tree.into_iter()
                            .map(|(_, node)| node.entry_hash);
                        todo_keys.extend(children);
                    }
                    Entry::Commit(commit) => {
                        todo_keys.push(commit.root_hash);
                    }
                }
            }
        }

        if reused_keys.len() > len && reused_keys[0].len() == 0 && todo_keys.len() == 0 {
            reused_keys.drain(..1);
            stores_stats.lock().unwrap().drain(..1);
            stores.write().unwrap().drain(..1);
        }
    }

    println!("MerkleStorage GC thread shut down!");
}

// when key is reused

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem;
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

    fn blob_serialized(value: Vec<u8>) -> Vec<u8> {
        bincode::serialize(&blob(value)).unwrap()
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

    #[test]
    fn test_stats() {
        let store = &mut empty_kvstore_gced(3);

        let kv1 = (entry_hash(&[1]), blob_serialized(vec![1]));
        let kv2 = (entry_hash(&[2]), blob_serialized(vec![1, 2]));
        let kv3 = (entry_hash(&[3]), blob_serialized(vec![1, 2, 3]));
        let kv4 = (entry_hash(&[4]), blob_serialized(vec![1, 2, 3, 4]));

        store.put(kv1.0.clone(), kv1.1.clone()).unwrap();
        store.put(kv2.0.clone(), kv2.1.clone()).unwrap();
        store.start_new_cycle();
        store.put(kv3.0.clone(), kv3.1.clone()).unwrap();
        store.start_new_cycle();
        store.put(kv4.0.clone(), kv4.1.clone()).unwrap();
        store.mark_reused(kv1.0.clone());

        store.wait_for_gc_finish();

        let stats: Vec<_> = store.get_stats().into_iter().rev().take(3).rev().collect();
        assert_eq!(stats[0].key_bytes, 64);
        assert_eq!(stats[0].value_bytes, size_of_vec(&kv1.1) + size_of_vec(&kv2.1));
        assert_eq!(stats[0].reused_keys_bytes, 96);

        assert_eq!(stats[1].key_bytes, 32);
        assert_eq!(stats[1].value_bytes, size_of_vec(&kv3.1));
        assert_eq!(stats[1].reused_keys_bytes, 0);

        assert_eq!(stats[2].key_bytes, 32);
        assert_eq!(stats[2].value_bytes, size_of_vec(&kv4.1));
        assert_eq!(stats[2].reused_keys_bytes, 0);

        assert_eq!(store.total_mem_usage_as_bytes(), vec![
            4 * mem::size_of::<EntryHash>(),
            96, // reused keys
            size_of_vec(&kv1.1),
            size_of_vec(&kv2.1),
            size_of_vec(&kv3.1),
            size_of_vec(&kv4.1),
        ].iter().sum::<usize>());

        store.start_new_cycle();
        store.wait_for_gc_finish();

        let stats = store.get_stats();
        assert_eq!(stats[0].key_bytes, 32);
        assert_eq!(stats[0].value_bytes, size_of_vec(&kv3.1));
        assert_eq!(stats[0].reused_keys_bytes, 0);

        assert_eq!(stats[1].key_bytes, 64);
        assert_eq!(stats[1].value_bytes, size_of_vec(&kv1.1) + size_of_vec(&kv4.1));
        assert_eq!(stats[1].reused_keys_bytes, 0);

        assert_eq!(stats[2].key_bytes, 0);
        assert_eq!(stats[2].value_bytes, 0);
        assert_eq!(stats[2].reused_keys_bytes, 0);

        assert_eq!(store.total_mem_usage_as_bytes(), vec![
            3 * mem::size_of::<EntryHash>(),
            size_of_vec(&kv1.1),
            size_of_vec(&kv3.1),
            size_of_vec(&kv4.1),
        ].iter().sum::<usize>());
    }
}
