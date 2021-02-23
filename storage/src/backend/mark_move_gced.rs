use std::collections::HashSet;

use std::mem;
use std::ops::{Deref, DerefMut};
use std::sync::{mpsc, Arc, Mutex, RwLock};
use rocksdb::WriteBatch;
use crate::MerkleStorage;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use crate::backend::InMemoryBackend;
use crate::persistent::database::{SimpleKeyValueStoreWithSchema, DBError, RocksDBStats};
use std::thread;
use std::time::Duration;

use crate::merkle_storage::{ContextValue, Entry, EntryHash};
use crate::storage_backend::{
    StorageBackend,
    StorageBackendError,
    StorageBackendStats,
};


/// Finds the value with hash `key` in one of the cycle stores (trying from newest to oldest)
fn stores_get<T, S>(stores: &S, key: &EntryHash) -> Option<ContextValue>
where
    T: SimpleKeyValueStoreWithSchema<MerkleStorage>,
    S: Deref<Target = Vec<T>>,
{
    stores
        .iter()
        .rev()
        .find_map(|store| store.get(key).unwrap_or(None))
}

/// Returns store index containing the entry with hash `key`  (trying from newest to oldest)
fn stores_containing<T, S>(stores: &S, key: &EntryHash) -> Option<usize>
where
    T: SimpleKeyValueStoreWithSchema<MerkleStorage>,
    S: Deref<Target = Vec<T>>,
{
    stores
        .iter()
        .enumerate()
        .find(|(_, store)| store.contains(key).unwrap_or(false))
        .map(|(index, _)| index)
}

/// Returns `true` if any of the stores contains an entry with hash `key`, and `false` otherwise
fn stores_contains<T, S>(stores: &S, key: &EntryHash) -> bool
where
    T: SimpleKeyValueStoreWithSchema<MerkleStorage>,
    S: Deref<Target = Vec<T>>,
{
    stores
        .iter()
        .rev()
        .any(|store| store.contains(key).unwrap_or(false))
}

/// Searches the stores for an entry with hash `key`, and if found, the value is deleted.
///
/// The return value is `None` if the value was not found, or Some((store_idx, value)) otherwise.
fn stores_delete<T, S>(stores: &mut S, key: &EntryHash) -> Option<(usize, ContextValue)>
where
    T: SimpleKeyValueStoreWithSchema<MerkleStorage>,
    S: DerefMut<Target = Vec<T>>,
{
    stores
        .iter_mut()
        .enumerate()
        .rev()
        .find_map(|(index, store)| {
            store
                .try_delete(key)
                .unwrap_or(None)
                .map(|value| (index, value))
        })
}

/// Commands used by MarkMoveGCed to interact with GC thread.
pub enum CmdMsg {
    StartNewCycle,
    Exit,
    MarkReused(EntryHash),
}

/// Garbage Collected Key Value Store
pub struct MarkMoveGCed<T: SimpleKeyValueStoreWithSchema<MerkleStorage>> {
    /// Stores for each cycle, older to newer
    stores: Arc<RwLock<Vec<T>>>,
    /// Stats for each store in archived stores
    stores_stats: Arc<Mutex<Vec<StorageBackendStats>>>,
    /// Current in-process cycle store
    current: T,
    /// Current in-process cycle store stats
    current_stats: StorageBackendStats,
    /// GC thread
    is_busy: Arc<AtomicBool>,
    msg_cnt: Arc<AtomicUsize>,
    _thread: thread::JoinHandle<()>,
    /// Channel to communicate with GC thread from main thread
    msg: Mutex<mpsc::Sender<CmdMsg>>,
}

impl<T: 'static + SimpleKeyValueStoreWithSchema<MerkleStorage> + Send + Sync + Default> MarkMoveGCed<T> {
    pub fn new(cycle_count: usize) -> Self {
        assert!(
            cycle_count > 1,
            "cycle_count less than 2 for MarkMoveGCed not supported"
        );

        let (tx, rx) = mpsc::channel();
        let stores = Arc::new(RwLock::new(
            (0..(cycle_count - 1)).map(|_| Default::default()).collect(),
        ));
        let msg_cnt = Arc::new(AtomicUsize::new(0));
        let msg_cnt_ref = msg_cnt.clone();
        let busy = Arc::new(AtomicBool::new(true));
        let busy_ref = busy.clone();
        let stores_stats = Arc::new(Mutex::new(vec![Default::default(); cycle_count - 1]));

        Self {
            stores: stores.clone(),
            stores_stats: stores_stats.clone(),
            _thread: thread::spawn(move || kvstore_gc_thread_fn(stores, stores_stats, rx, busy.clone(), msg_cnt)),
            msg: Mutex::new(tx),
            is_busy: busy_ref,
            msg_cnt: msg_cnt_ref,
            current: Default::default(),
            current_stats: Default::default(),
        }
    }

    /// Finds an entry with hash `key` in one of the archived cycle stores (trying from newest to oldest)
    fn stores_get(&self, key: &EntryHash) -> Option<ContextValue> {
        stores_get(&self.stores.read().unwrap(), key)
    }

    /// Returns `true` if any of the archived cycle stores contains a value with hash `key`, and `false` otherwise
    fn stores_contains(&self, key: &EntryHash) -> bool {
        stores_contains(&self.stores.read().unwrap(), key)
    }

    fn get_stats(&self) -> Vec<StorageBackendStats> {
        self.stores_stats
            .lock()
            .unwrap()
            .iter()
            .chain(vec![&self.current_stats])
            .cloned()
            .collect()
    }
}

impl<T: 'static + SimpleKeyValueStoreWithSchema<MerkleStorage> + Send + Sync + Default> SimpleKeyValueStoreWithSchema<MerkleStorage> for MarkMoveGCed<T> {
    fn put(& self, key: &EntryHash, value: &ContextValue) -> Result<(), DBError> {
        self.current.put(key,value)
    }

    fn delete(&self, key: &EntryHash) -> Result<(), DBError> {
        self.current.delete(key)
    }

    fn merge(&self, key: &EntryHash, value: &ContextValue) -> Result<(), DBError> {
        self.current.merge(key, value)
    }

    fn get(&self, key: &EntryHash) -> Result<Option<ContextValue>, DBError> {
        Ok(self.current.get(key)?.or_else(|| self.stores_get(key)))
    }

    fn contains(&self, key: &EntryHash) -> Result<bool, DBError> {
        Ok(self.current.contains(key)? || self.stores_contains(key))
    }

    fn put_batch(
        &self,
        batch: &mut WriteBatch,
        key: &EntryHash,
        value: &ContextValue,
    ) -> Result<(), DBError> {
        unimplemented!();
    }

    fn write_batch(&self, batch: WriteBatch) -> Result<(), DBError> {
        unimplemented!();
    }

    fn get_stats(&self) -> Result<RocksDBStats, DBError> {
        // TODO iter for the whole vector of stats
        self.current.get_stats()
    }

    fn retain(&self, predicate: &dyn Fn(&EntryHash) -> bool) -> Result<(), DBError> {
        self.current.retain(predicate)
    }
}

// impl<T: 'static + StorageBackend + Default> StorageBackend for MarkMoveGCed<T> {
//     fn is_persisted(&self) -> bool {
//         self.current.is_persisted()
//     }
//
//     fn total_get_mem_usage(&self) -> Result<usize,StorageBackendError>{
//         let stats: StorageBackendStats = self.get_stats().iter().sum();
//         Ok(stats.total_as_bytes())
//     }
//
//     /// Get an entry with hash `key` from the current in-progress cycle store,
//     /// otherwise find and get from archived cycle stores
//     fn get(&self, key: &EntryHash) -> Result<Option<ContextValue>, StorageBackendError> {
//         Ok(self.current.get(key)?.or_else(|| self.stores_get(key)))
//     }
//
//     /// Checks if an entry with hash `key` exists in any of the cycle stores
//     fn contains(&self, key: &EntryHash) -> Result<bool, StorageBackendError> {
//         Ok(self.current.contains(key)? || self.stores_contains(key))
//     }
//
//     fn put(&mut self, key: &EntryHash, value: ContextValue) -> Result<bool, StorageBackendError> {
//         let measurement = StorageBackendStats::from((key, &value));
//         let was_added = self.current.put(key, value)?;
//
//         if was_added {
//             self.current_stats += measurement;
//         }
//
//         Ok(was_added)
//     }
//
//     fn merge(&mut self, key: &EntryHash, value: ContextValue) -> Result<(), StorageBackendError> {
//         self.current.merge(key, value)
//     }
//
//     fn delete(&mut self, key: &EntryHash) -> Result<Option<ContextValue>, StorageBackendError> {
//         self.current.delete(key)
//     }
//
//     /// Marks an entry as "reused" in the current cycle.
//     ///
//     /// Entries that are reused/referenced in current cycle
//     /// will be preserved after garbage collection.
//     fn mark_reused(&mut self, key: EntryHash) {
//         self.msg_cnt.fetch_add(1, Ordering::Acquire);
//         let _ = self.msg.lock().unwrap().send(CmdMsg::MarkReused(key));
//     }
//
//     /// Not needed/implemented.
//     // TODO: Maybe this method should go into separate trait?
//     fn retain(&mut self, _pred: HashSet<EntryHash>) -> Result<(), StorageBackendError> {
//         unimplemented!()
//     }
//
//     /// Starts a new cycle.
//     ///
//     /// Garbage collector will start collecting the oldest cycle.
//     fn start_new_cycle(&mut self, _last_commit_hash: Option<EntryHash>) {
//         self.msg_cnt.fetch_add(1, Ordering::Acquire);
//         self.stores_stats
//             .lock()
//             .unwrap()
//             .push(mem::take(&mut self.current_stats));
//         self.stores
//             .write()
//             .unwrap()
//             .push(mem::take(&mut self.current));
//         let _ = self.msg.lock().unwrap().send(CmdMsg::StartNewCycle);
//     }
//
//     /// Waits for garbage collector to finish collecting the oldest cycle.
//     /// [used only for testing purposes]
//     fn wait_for_gc_finish(&self) {
//         while self.is_busy.load(Ordering::Acquire) || self.msg_cnt.load(Ordering::Acquire) > 0{
//             thread::sleep(Duration::from_millis(20));
//         }
//     }
//
// }

/// Garbage collector main function
fn kvstore_gc_thread_fn<T: SimpleKeyValueStoreWithSchema<MerkleStorage>>(
    stores: Arc<RwLock<Vec<T>>>,
    stores_stats: Arc<Mutex<Vec<StorageBackendStats>>>,
    rx: mpsc::Receiver<CmdMsg>,
    is_busy: Arc<AtomicBool>,
    msg_cnt: Arc<AtomicUsize>
) {
    // number of preserved archived cycles
    let len = stores.read().unwrap().len();
    // maintains incoming references (hashes) for each archived cycle store.
    let mut reused_keys = vec![HashSet::new(); len];
    // this is filled when garbage collection starts and it contains keys
    // that are reused from the oldest store and needs to be moved to newer
    // stores so that after destroying oldest store they are preserved.
    let mut todo_keys: Vec<EntryHash> = vec![];
    let mut received_exit_msg = false;

    loop {
        // wait (block) for main thread events if there are no items to garbage collect
        let wait_for_events = reused_keys.len() == len && !received_exit_msg;

        // check if there are not processed eventsda
        is_busy.swap(
                msg_cnt.load(Ordering::Acquire) > 0
                || ! todo_keys.is_empty() 
                || reused_keys.len() > len, Ordering::Acquire);

        let msg = if wait_for_events {
            match rx.recv() {
                Ok(value) => Some(value),
                Err(_) => {
                    return;
                }
            }
        } else {
            match rx.try_recv() {
                Ok(value) => Some(value),
                Err(_) => None,
            }
        };

        // process messages received from the main thread
        match msg {
            Some(CmdMsg::StartNewCycle) => {
                // new cycle started, we add a new reused/referenced keys HashSet for it
                reused_keys.push(Default::default());
                msg_cnt.fetch_sub(1, Ordering::Acquire);
            }
            Some(CmdMsg::Exit) => {
                received_exit_msg = true;
            }
            Some(CmdMsg::MarkReused(key)) => {
                if let Some(index) = stores_containing(&stores.read().unwrap(), &key) {
                    // only way index can be greater than reused_keys.len() is if GC thread
                    // lags behind (gc has pending 1-2 cycles to collect). When we still haven't
                    // received event from main thread that new cycle has started, yet it has.
                    // So we might receive `key` that was only in `current` store (when this event
                    // was sent by main thread). So if gc had't lagged behind, we wouldn't have found
                    // entry with that `key`. So this entry shouldn't be marked as reused.
                    if index < reused_keys.len() {
                        let keys = &mut reused_keys[index];
                        if keys.insert(key) {
                            stores_stats.lock().unwrap()[index].update_reused_keys(keys);
                        }
                    }
                }
                msg_cnt.fetch_sub(1, Ordering::Acquire);
            }
            None => {
                // we exit only if there are no remaining keys to be processed
                if received_exit_msg && todo_keys.is_empty() && reused_keys.len() == len {
                    is_busy.swap(false, Ordering::Acquire);
                    return;
                }
            }
        }

        // if reused_keys.len() > len  that means that we need to start garbage collecting oldest cycle,
        // reused_keys[0].len() > 0  If no keys are reused we can just drop the cycle.
        if reused_keys.len() > len && ! reused_keys[0].is_empty() {
            todo_keys.extend(mem::take(&mut reused_keys[0]).into_iter());
        }

        if !todo_keys.is_empty() {
            let mut stats = stores_stats.lock().unwrap();
            let mut stores = stores.write().unwrap();

            // it will block if new cycle begins and we still haven't garbage collected prev cycle.
            // Higher the max_iter (2048) will be, longer gc will block the main thread, but GC will
            // finish faster and will lag behind less.
            // Smaller the max_iter (2048) will be, longer it will take for gc to finish and it will
            // lag behind more (which will increase memory usage), but it will slow down main thread less
            // as the lock will be released on more frequent intervals.
            // TODO: it would be more optimized if this number can change dynamically based on
            // how much gc lags behind and such different parameters.
            let max_iter = if stores.len() <= 1 + len {
                2048
            } else {
                usize::MAX
            };

            for _ in 0..max_iter {
                let key = match todo_keys.pop() {
                    Some(key) => key,
                    None => break,
                };

                let (store_index, entry_bytes) = match stores_delete(&mut stores, &key) {
                    Some(result) => result,
                    // it's possible entry was deleted already when iterating on some other root during gc.
                    // So it's perfectly normal if referenced entry isn't found.
                    None => continue,
                };

                let stat: StorageBackendStats = (&key, &entry_bytes).into();
                stats[store_index] -= &stat;

                let entry: Entry = match bincode::deserialize(&entry_bytes) {
                    Ok(value) => value,
                    Err(err) => {
                        eprintln!(
                            "MerkleStorage GC: error while decerializing entry: {:?}",
                            err
                        );
                        continue;
                    }
                };

                // TODO: it would be better if we would move entries to the newest store
                // they were referenced in. This way entries won't live longer then they
                // have to (unlike how its now). This can be achieved if we keep cycle
                // information with `reused_keys`. So if it is Map instead of Set and
                // and we store maximum cycle in which it was referenced in as a value.
                // Then we can move each entry to a given store based on that value.
                if let Err(err) = stores.last_mut().unwrap().put(&key.clone(), &entry_bytes) {
                    eprintln!(
                        "MerkleStorage GC: error while adding entry to store: {:?}",
                        err
                    );
                } else {
                    // and update the stats for that store
                    *stats.last_mut().unwrap() += &stat;
                }

                match entry {
                    Entry::Blob(_) => {}
                    Entry::Tree(tree) => {
                        let children = tree.into_iter().map(|(_, node)| node.entry_hash);
                        todo_keys.extend(children);
                    }
                    Entry::Commit(commit) => {
                        todo_keys.push(commit.root_hash);
                    }
                }
            }
        }

        if reused_keys.len() > len && reused_keys[0].is_empty() && todo_keys.is_empty(){
            drop(reused_keys.drain(..1));
            drop(stores_stats.lock().unwrap().drain(..1));
            drop(stores.write().unwrap().drain(..1));
        }
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use crate::backend::BTreeMapBackend;
//     use crate::storage_backend::size_of_vec;
//     use std::convert::TryFrom;
//
//     fn empty_kvstore_gced(cycle_count: usize) -> MarkMoveGCed<BTreeMapBackend> {
//         MarkMoveGCed::new(cycle_count)
//     }
//
//     fn entry_hash(key: &[u8]) -> EntryHash {
//         assert!(key.len() < 32);
//         let bytes: Vec<u8> = key
//             .iter()
//             .chain(std::iter::repeat(&0u8))
//             .take(32)
//             .cloned()
//             .collect();
//
//         EntryHash::try_from(bytes).unwrap()
//     }
//
//     fn blob(value: Vec<u8>) -> Entry {
//         Entry::Blob(value)
//     }
//
//     fn blob_serialized(value: Vec<u8>) -> Vec<u8> {
//         bincode::serialize(&blob(value)).unwrap()
//     }
//
//     fn get<T: 'static + StorageBackend + Default>(store: &MarkMoveGCed<T>, key: &[u8]) -> Option<Entry> {
//         store
//             .get(&entry_hash(key))
//             .unwrap()
//             .map(|x| bincode::deserialize(&x[..]).unwrap())
//     }
//
//     fn put<T: 'static + StorageBackend + Default>(store: &mut MarkMoveGCed<T>, key: &[u8], value: Entry) {
//         store
//             .put(&entry_hash(key), bincode::serialize(&value).unwrap())
//             .unwrap();
//     }
//
//     fn mark_reused<T: 'static + StorageBackend + Default>(store: &mut MarkMoveGCed<T>, key: &[u8]) {
//         store.mark_reused(entry_hash(key));
//     }
//
//     #[test]
//     fn test_key_reused_exists() {
//         let store = &mut empty_kvstore_gced(3);
//
//         put(store, &[1], blob(vec![1]));
//         put(store, &[2], blob(vec![2]));
//         store.start_new_cycle(None);
//         put(store, &[3], blob(vec![3]));
//         store.start_new_cycle(None);
//         put(store, &[4], blob(vec![4]));
//         mark_reused(store, &[1]);
//         store.start_new_cycle(None);
//
//         store.wait_for_gc_finish();
//
//         assert_eq!(get(store, &[1]), Some(blob(vec![1])));
//         assert_eq!(get(store, &[2]), None);
//         assert_eq!(get(store, &[3]), Some(blob(vec![3])));
//         assert_eq!(get(store, &[4]), Some(blob(vec![4])));
//     }
//
//     #[test]
//     fn test_stats() {
//         let store = &mut empty_kvstore_gced(3);
//
//         let kv1 = (entry_hash(&[1]), blob_serialized(vec![1]));
//         let kv2 = (entry_hash(&[2]), blob_serialized(vec![1, 2]));
//         let kv3 = (entry_hash(&[3]), blob_serialized(vec![1, 2, 3]));
//         let kv4 = (entry_hash(&[4]), blob_serialized(vec![1, 2, 3, 4]));
//
//         store.wait_for_gc_finish();
//
//         store.put(&kv1.0.clone(), kv1.1.clone()).unwrap();
//         store.put(&kv2.0.clone(), kv2.1.clone()).unwrap();
//         store.start_new_cycle(None);
//         store.wait_for_gc_finish();
//
//         store.put(&kv3.0.clone(), kv3.1.clone()).unwrap();
//         store.start_new_cycle(None);
//         store.wait_for_gc_finish();
//
//         store.put(&kv4.0.clone(), kv4.1.clone()).unwrap();
//         store.mark_reused(kv1.0);
//         store.wait_for_gc_finish();
//
//
//         let stats: Vec<_> = store.get_stats().into_iter().rev().take(3).rev().collect();
//         assert_eq!(stats[0].key_bytes, 64);
//         assert_eq!(
//             stats[0].value_bytes,
//             size_of_vec(&kv1.1) + size_of_vec(&kv2.1)
//         );
//         assert_eq!(stats[0].reused_keys_bytes, 96);
//
//         assert_eq!(stats[1].key_bytes, 32);
//         assert_eq!(stats[1].value_bytes, size_of_vec(&kv3.1));
//         assert_eq!(stats[1].reused_keys_bytes, 0);
//
//         assert_eq!(stats[2].key_bytes, 32);
//         assert_eq!(stats[2].value_bytes, size_of_vec(&kv4.1));
//         assert_eq!(stats[2].reused_keys_bytes, 0);
//
//         assert_eq!(
//             store.total_get_mem_usage().unwrap(),
//             vec![
//                 4 * mem::size_of::<EntryHash>(),
//                 96, // reused keys
//                 size_of_vec(&kv1.1),
//                 size_of_vec(&kv2.1),
//                 size_of_vec(&kv3.1),
//                 size_of_vec(&kv4.1),
//             ]
//             .iter()
//             .sum::<usize>()
//         );
//
//         store.start_new_cycle(None);
//         store.wait_for_gc_finish();
//
//         let stats = store.get_stats();
//         assert_eq!(stats[0].key_bytes, 32);
//         assert_eq!(stats[0].value_bytes, size_of_vec(&kv3.1));
//         assert_eq!(stats[0].reused_keys_bytes, 0);
//
//         assert_eq!(stats[1].key_bytes, 64);
//         assert_eq!(
//             stats[1].value_bytes,
//             size_of_vec(&kv1.1) + size_of_vec(&kv4.1)
//         );
//         assert_eq!(stats[1].reused_keys_bytes, 0);
//
//         assert_eq!(stats[2].key_bytes, 0);
//         assert_eq!(stats[2].value_bytes, 0);
//         assert_eq!(stats[2].reused_keys_bytes, 0);
//
//         assert_eq!(
//             store.total_get_mem_usage().unwrap(),
//             vec![
//                 3 * mem::size_of::<EntryHash>(),
//                 size_of_vec(&kv1.1),
//                 size_of_vec(&kv3.1),
//                 size_of_vec(&kv4.1),
//             ]
//             .iter()
//             .sum::<usize>()
//         );
//     }
// }
