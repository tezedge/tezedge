use std::{
    borrow::Cow,
    collections::{hash_map::DefaultHasher, BTreeMap, HashMap, VecDeque},
    convert::TryFrom,
    hash::Hasher,
    num::NonZeroUsize,
    sync::Arc,
};

use crossbeam_channel::{Receiver, Sender};
use crypto::hash::ContextHash;
use serde::{Deserialize, Serialize};

use super::entries::Entries;
use crate::{
    hash::EntryHash,
    persistent::{DBError, Flushable, KeyValueStoreBackend, Persistable},
    working_tree::Entry,
};

use tezos_spsc::{Consumer, Producer};

const PRESERVE_CYCLE_COUNT: usize = 7;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct HashId(NonZeroUsize); // NonZeroUsize so that `Option<HashId>` is 8 bytes

impl Into<usize> for HashId {
    fn into(self) -> usize {
        self.0.get().checked_sub(1).unwrap()
    }
}

impl From<usize> for HashId {
    fn from(v: usize) -> Self {
        let v = v.checked_add(1).unwrap();
        HashId(NonZeroUsize::new(v).unwrap())
    }
}

impl HashId {
    pub(crate) fn invalid() -> Self {
        Self(NonZeroUsize::new(usize::MAX).unwrap())
    }
}

#[derive(Debug)]
pub struct HashValueStore {
    hashes: Entries<HashId, EntryHash>,
    values: Entries<HashId, Option<Arc<[u8]>>>,
    free_ids: Consumer<HashId>,
    new_ids: Vec<HashId>,
}

pub struct VacantEntryHash<'a> {
    entry: Option<&'a mut EntryHash>,
    hash_id: HashId,
}

impl<'a> VacantEntryHash<'a> {
    pub(crate) fn invalid() -> Self {
        Self {
            entry: None,
            hash_id: HashId::invalid(),
        }
    }

    pub(crate) fn write_with<F>(self, fun: F) -> HashId
    where
        F: FnOnce(&mut EntryHash),
    {
        if let Some(entry) = self.entry {
            fun(entry)
        };
        self.hash_id
    }
}

impl HashValueStore {
    fn new(consumer: Consumer<HashId>) -> Self {
        Self {
            hashes: Entries::new(),
            values: Entries::new(),
            free_ids: consumer,
            new_ids: Vec::with_capacity(1024),
        }
    }

    pub(crate) fn get_vacant_entry_hash(&mut self) -> VacantEntryHash {
        let (hash_id, entry) = if let Some(free_id) = self.free_ids.pop().ok() {
            self.values[free_id] = None;
            (free_id, &mut self.hashes[free_id])
        } else {
            self.hashes.get_vacant_entry()
        };
        self.new_ids.push(hash_id);

        VacantEntryHash {
            entry: Some(entry),
            hash_id,
        }
    }

    pub(crate) fn insert_value_at(&mut self, hash_id: HashId, value: Arc<[u8]>) {
        self.values.insert_at(hash_id, Some(value));
    }

    pub(crate) fn get_hash(&self, hash_id: HashId) -> Option<&EntryHash> {
        self.hashes.get(hash_id)
    }

    pub(crate) fn get_value(&self, hash_id: HashId) -> Option<&[u8]> {
        self.values.get(hash_id)?.as_ref().map(|v| v.as_ref())
    }

    pub(crate) fn contains(&self, hash_id: HashId) -> bool {
        self.values.get(hash_id).unwrap_or(&None).is_some()
    }

    fn take_new_ids(&mut self) -> Vec<HashId> {
        let new_ids = self.new_ids.clone();
        self.new_ids.clear();
        new_ids
    }
}

pub struct Repository {
    current_cycle: BTreeMap<HashId, Option<Arc<[u8]>>>,
    pub hashes: HashValueStore,
    sender: Sender<Command>,
    pub context_hashes: HashMap<u64, HashId>,
    context_hashes_cycles: VecDeque<Vec<u64>>,
}

struct GCThread {
    cycles: Cycles,
    free_ids: Producer<HashId>,
    recv: Receiver<Command>,
    pending: Vec<HashId>,
}

enum Command {
    StartNewCycle {
        values_in_cycle: BTreeMap<HashId, Option<Arc<[u8]>>>,
        new_ids: Vec<HashId>,
    },
    MarkReused {
        reused: Vec<HashId>,
    },
    Exit,
}

struct Cycles {
    list: VecDeque<BTreeMap<HashId, Option<Arc<[u8]>>>>,
}

impl Default for Cycles {
    fn default() -> Self {
        let mut list = VecDeque::with_capacity(PRESERVE_CYCLE_COUNT);

        for _ in 0..PRESERVE_CYCLE_COUNT {
            list.push_back(Default::default());
        }

        Self { list }
    }
}

impl Cycles {
    fn move_to_last_cycle(&mut self, hash_id: HashId) -> Option<Arc<[u8]>> {
        let len = self.list.len();
        let mut value = None;

        for store in &mut self.list.iter_mut().take(len - 1) {
            if let Some(item) = store.remove(&hash_id).flatten() {
                value = Some(item);
            };
        }

        let value = value?;
        self.list
            .back_mut()
            .unwrap()
            .insert(hash_id, Some(Arc::clone(&value)));
        return Some(value);
    }

    fn roll(&mut self, new_cycle: BTreeMap<HashId, Option<Arc<[u8]>>>) -> Vec<HashId> {
        let unused = self.list.pop_front().unwrap();
        self.list.push_back(new_cycle);

        let mut vec = Vec::with_capacity(unused.len());
        for id in unused.keys() {
            vec.push(*id);
        }
        vec
    }
}

impl Default for Repository {
    fn default() -> Self {
        Self::new()
    }
}

impl super::GarbageCollector for Repository {
    fn new_cycle_started(&mut self) -> Result<(), super::GarbageCollectionError> {
        self.new_cycle_started();
        Ok(())
    }

    fn block_applied(
        &mut self,
        referenced_older_entries: Vec<HashId>,
    ) -> Result<(), super::GarbageCollectionError> {
        self.block_applied(referenced_older_entries);
        Ok(())
    }
}

impl Flushable for Repository {
    fn flush(&self) -> Result<(), failure::Error> {
        Ok(())
    }
}

impl Persistable for Repository {
    fn is_persistent(&self) -> bool {
        false
    }
}

impl KeyValueStoreBackend for Repository {
    fn write_batch(&mut self, batch: Vec<(HashId, Arc<[u8]>)>) -> Result<(), DBError> {
        self.write_batch(batch);
        Ok(())
    }

    fn contains(&self, hash_id: HashId) -> Result<bool, DBError> {
        self.contains(hash_id)
    }

    fn put_context_hash(&mut self, hash_id: HashId) -> Result<ContextHash, DBError> {
        Ok(self.put_context_hash_impl(hash_id))
    }

    fn get_context_hash(&self, context_hash: &ContextHash) -> Result<Option<HashId>, DBError> {
        Ok(self.get_context_hash_impl(context_hash))
    }

    fn get_hash(&self, hash_id: HashId) -> Result<Option<Cow<EntryHash>>, DBError> {
        Ok(self.get_hash(hash_id).map(|h| Cow::Borrowed(h)))
    }

    fn get_value(&self, hash_id: HashId) -> Result<Option<Cow<[u8]>>, DBError> {
        Ok(self.get_value(hash_id).map(|v| Cow::Borrowed(v)))
    }

    fn get_vacant_entry_hash(&mut self) -> Result<VacantEntryHash, DBError> {
        Ok(self.get_vacant_entry_hash())
    }
}

impl Repository {
    pub fn new() -> Self {
        let (sender, recv) = crossbeam_channel::unbounded();
        let (prod, cons) = tezos_spsc::bounded(2_000_000);

        std::thread::spawn(move || {
            GCThread {
                cycles: Cycles::default(),
                recv,
                free_ids: prod,
                pending: Vec::new(),
            }
            .run()
        });

        let current_cycle = Default::default();
        let hashes = HashValueStore::new(cons);
        let context_hashes = Default::default();

        let mut context_hashes_cycles = VecDeque::with_capacity(PRESERVE_CYCLE_COUNT);
        for _ in 0..PRESERVE_CYCLE_COUNT {
            context_hashes_cycles.push_back(Default::default())
        }

        Self {
            current_cycle,
            hashes,
            sender,
            context_hashes,
            context_hashes_cycles,
        }
    }

    pub(crate) fn get_vacant_entry_hash(&mut self) -> VacantEntryHash {
        self.hashes.get_vacant_entry_hash()
    }

    pub(crate) fn get_hash(&self, hash_id: HashId) -> Option<&EntryHash> {
        self.hashes.get_hash(hash_id)
    }

    pub(crate) fn get_value(&self, hash_id: HashId) -> Option<&[u8]> {
        self.hashes.get_value(hash_id)
    }

    fn contains(&self, hash_id: HashId) -> Result<bool, DBError> {
        Ok(self.hashes.contains(hash_id))
    }

    pub fn write_batch(&mut self, batch: Vec<(HashId, Arc<[u8]>)>) {
        for (hash_id, value) in batch {
            self.hashes.insert_value_at(hash_id, Arc::clone(&value));
            self.current_cycle.insert(hash_id, Some(value));
        }
    }

    pub fn new_cycle_started(&mut self) {
        let values_in_cycle = std::mem::take(&mut self.current_cycle);
        let new_ids = self.hashes.take_new_ids();

        self.sender
            .send(Command::StartNewCycle {
                values_in_cycle,
                new_ids,
            })
            .unwrap();

        let unused = self.context_hashes_cycles.pop_front().unwrap();
        for hash in unused {
            self.context_hashes.remove(&hash);
        }
        self.context_hashes_cycles.push_back(Default::default());
    }

    pub fn block_applied(&mut self, reused: Vec<HashId>) {
        self.sender.send(Command::MarkReused { reused }).unwrap()
    }

    pub fn get_context_hash_impl(&self, context_hash: &ContextHash) -> Option<HashId> {
        let mut hasher = DefaultHasher::new();
        hasher.write(context_hash.as_ref());
        let hashed = hasher.finish();

        self.context_hashes.get(&hashed).cloned()
    }

    pub fn put_context_hash_impl(&mut self, commit_hash_id: HashId) -> ContextHash {
        let commit_hash = self.hashes.get_hash(commit_hash_id).unwrap();

        let mut hasher = DefaultHasher::new();
        hasher.write(&commit_hash[..]);
        let hashed = hasher.finish();

        self.context_hashes.insert(hashed, commit_hash_id);
        self.context_hashes_cycles.back_mut().unwrap().push(hashed);

        ContextHash::try_from(&commit_hash[..]).unwrap()
    }

    #[cfg(test)]
    pub(crate) fn put_entry_hash(&mut self, entry_hash: EntryHash) -> HashId {
        let vacant = self.get_vacant_entry_hash();
        vacant.write_with(|entry| *entry = entry_hash)
    }
}

impl GCThread {
    fn run(mut self) {
        while let Ok(msg) = self.recv.recv() {
            match msg {
                Command::StartNewCycle {
                    values_in_cycle,
                    new_ids,
                } => self.start_new_cycle(values_in_cycle, new_ids),
                Command::MarkReused { reused } => self.mark_reused(reused),
                Command::Exit => {
                    return;
                }
            }
        }
    }

    fn start_new_cycle(
        &mut self,
        mut new_cycle: BTreeMap<HashId, Option<Arc<[u8]>>>,
        new_ids: Vec<HashId>,
    ) {
        for hash_id in new_ids.into_iter() {
            new_cycle.entry(hash_id).or_insert(None);
        }
        let unused = self.cycles.roll(new_cycle);
        self.send_unused(unused);
    }

    /// Notify the main thread that the ids are free to reused
    fn send_unused(&mut self, unused: Vec<HashId>) {
        let unused_length = unused.len();
        let navailable = self.free_ids.available();

        let (to_send, pending) = if navailable < unused_length {
            unused.split_at(navailable)
        } else {
            (&unused[..], &[][..])
        };

        self.free_ids.push_slice(&to_send).unwrap();

        if !pending.is_empty() {
            self.pending.extend_from_slice(pending);
        }
    }

    fn send_pending(&mut self) {
        if self.pending.is_empty() {
            return;
        }

        let navailable = self.free_ids.available();
        if navailable == 0 {
            return;
        }

        let n_to_send = navailable.min(self.pending.len());
        let start = self.pending.len() - n_to_send;
        let to_send = &self.pending[start..];

        self.free_ids.push_slice(&to_send).unwrap();
        self.pending.truncate(start);
    }

    fn mark_reused(&mut self, mut reused: Vec<HashId>) {
        while let Some(hash_id) = reused.pop() {
            let value = match self.cycles.move_to_last_cycle(hash_id) {
                Some(v) => v,
                None => continue,
            };

            let entry: Entry = match bincode::deserialize(&value) {
                Ok(value) => value,
                Err(err) => {
                    eprintln!("WorkingTree GC: error while decerializing entry: {:?}", err);
                    continue;
                }
            };

            match entry {
                Entry::Blob(_) => {}
                Entry::Tree(tree) => {
                    // Push every entry in this directory
                    for node in tree.values() {
                        reused.push(node.entry_hash.get().unwrap().clone());
                    }
                }
                Entry::Commit(commit) => {
                    // Push the root tree for this commit
                    reused.push(commit.root_hash);
                }
            }
        }
        self.send_pending();
    }
}
