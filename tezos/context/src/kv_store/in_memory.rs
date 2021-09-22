// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Implementation of an in-memory repository.

use std::{
    borrow::Cow,
    collections::{hash_map::DefaultHasher, BTreeMap, VecDeque},
    hash::Hasher,
    mem::size_of,
    sync::{atomic::Ordering, Arc},
    thread::JoinHandle,
};

use crossbeam_channel::Sender;
use crypto::hash::ContextHash;
use tezos_timing::RepositoryMemoryUsage;

use crate::{
    gc::{
        worker::{Command, Cycles, GCThread, GC_PENDING_HASHIDS, PRESERVE_CYCLE_COUNT},
        GarbageCollectionError, GarbageCollector,
    },
    hash::ObjectHash,
    persistent::{DBError, Flushable, KeyValueStoreBackend, Persistable},
    working_tree::{
        shape::{DirectoryShapeId, DirectoryShapes, ShapeStrings},
        storage::DirEntryId,
        string_interner::{StringId, StringInterner},
    },
    Map,
};

use tezos_spsc::Consumer;

use super::{index_map::IndexMap, HashIdError};
use super::{HashId, VacantObjectHash};

#[derive(Debug)]
pub struct HashValueStore {
    hashes: IndexMap<HashId, ObjectHash>,
    values: IndexMap<HashId, Option<Arc<[u8]>>>,
    free_ids: Option<Consumer<HashId>>,
    new_ids: Vec<HashId>,
    values_bytes: usize,
}

impl HashValueStore {
    pub(crate) fn new<T>(consumer: T) -> Self
    where
        T: Into<Option<Consumer<HashId>>>,
    {
        Self {
            hashes: IndexMap::new(),
            values: IndexMap::new(),
            free_ids: consumer.into(),
            new_ids: Vec::with_capacity(1024),
            values_bytes: 0,
        }
    }

    pub fn get_memory_usage(&self) -> RepositoryMemoryUsage {
        let values_bytes = self.values_bytes;
        let values_capacity = self.values.capacity();
        let hashes_capacity = self.hashes.capacity();
        let total_bytes = values_bytes
            .saturating_add(values_capacity * size_of::<Option<Arc<[u8]>>>())
            .saturating_add(values_capacity * 16) // Each `Arc` has 16 extra bytes for the counters
            .saturating_add(hashes_capacity * size_of::<ObjectHash>());

        RepositoryMemoryUsage {
            values_bytes,
            values_capacity,
            values_length: self.values.len(),
            hashes_capacity,
            hashes_length: self.hashes.len(),
            total_bytes,
            npending_free_ids: self.free_ids.as_ref().map(|c| c.len()).unwrap_or(0),
            gc_npending_free_ids: GC_PENDING_HASHIDS.load(Ordering::Acquire),
            nshapes: 0,
        }
    }

    pub(crate) fn clear(&mut self) {
        *self = Self {
            hashes: IndexMap::new(),
            values: IndexMap::new(),
            free_ids: self.free_ids.take(),
            new_ids: Vec::new(),
            values_bytes: 0,
        }
    }

    pub(crate) fn get_vacant_object_hash(&mut self) -> Result<VacantObjectHash, HashIdError> {
        let (hash_id, entry) = if let Some(free_id) = self.get_free_id() {
            if let Some(old_value) = self.values.set(free_id, None)? {
                self.values_bytes = self.values_bytes.saturating_sub(old_value.len());
            }
            (free_id, self.hashes.get_mut(free_id)?.ok_or(HashIdError)?)
        } else {
            self.hashes.get_vacant_entry()?
        };
        self.new_ids.push(hash_id);

        todo!()
        // Ok(VacantObjectHash {
        //     entry: Some(entry),
        //     hash_id,
        // })
    }

    fn get_free_id(&mut self) -> Option<HashId> {
        self.free_ids.as_mut()?.pop().ok()
    }

    pub(crate) fn insert_value_at(
        &mut self,
        hash_id: HashId,
        value: Arc<[u8]>,
    ) -> Result<(), HashIdError> {
        self.values_bytes = self.values_bytes.saturating_add(value.len());
        if let Some(old) = self.values.insert_at(hash_id, Some(value))? {
            self.values_bytes = self.values_bytes.saturating_sub(old.len());
        }
        Ok(())
    }

    pub(crate) fn get_hash(&self, hash_id: HashId) -> Result<Option<&ObjectHash>, HashIdError> {
        self.hashes.get(hash_id)
    }

    pub(crate) fn get_value(&self, hash_id: HashId) -> Result<Option<&[u8]>, HashIdError> {
        match self.values.get(hash_id)? {
            Some(value) => Ok(value.as_ref().map(|v| v.as_ref())),
            None => Ok(None),
        }
    }

    pub(crate) fn contains(&self, hash_id: HashId) -> Result<bool, HashIdError> {
        Ok(self.values.get(hash_id)?.unwrap_or(&None).is_some())
    }

    fn take_new_ids(&mut self) -> Vec<HashId> {
        let new_ids = self.new_ids.clone();
        self.new_ids.clear();
        new_ids
    }
}

pub struct InMemory {
    current_cycle: BTreeMap<HashId, Option<Arc<[u8]>>>,
    pub hashes: HashValueStore,
    sender: Option<Sender<Command>>,
    pub context_hashes: Map<u64, HashId>,
    context_hashes_cycles: VecDeque<Vec<u64>>,
    thread_handle: Option<JoinHandle<()>>,
    shapes: DirectoryShapes,
    string_interner: StringInterner,
}

impl GarbageCollector for InMemory {
    fn new_cycle_started(&mut self) -> Result<(), GarbageCollectionError> {
        self.new_cycle_started();
        Ok(())
    }

    fn block_applied(
        &mut self,
        referenced_older_objects: Vec<HashId>,
    ) -> Result<(), GarbageCollectionError> {
        self.block_applied(referenced_older_objects);
        Ok(())
    }
}

impl Flushable for InMemory {
    fn flush(&self) -> Result<(), anyhow::Error> {
        Ok(())
    }
}

impl Persistable for InMemory {
    fn is_persistent(&self) -> bool {
        false
    }
}

impl KeyValueStoreBackend for InMemory {
    fn write_batch(&mut self, batch: Vec<(HashId, Arc<[u8]>)>) -> Result<(), DBError> {
        self.write_batch(batch)
    }

    fn contains(&self, hash_id: HashId) -> Result<bool, DBError> {
        self.contains(hash_id)
    }

    fn put_context_hash(&mut self, hash_id: HashId, _offset: u64) -> Result<(), DBError> {
        self.put_context_hash_impl(hash_id)
    }

    fn get_context_hash(&self, context_hash: &ContextHash) -> Result<Option<(HashId, u64)>, DBError> {
        Ok(self.get_context_hash_impl(context_hash))
    }

    fn get_hash(&self, hash_id: HashId) -> Result<Option<Cow<ObjectHash>>, DBError> {
        Ok(self.get_hash(hash_id)?.map(Cow::Borrowed))
    }

    fn get_value(&self, hash_id: HashId) -> Result<Option<Cow<[u8]>>, DBError> {
        Ok(self.get_value(hash_id)?.map(Cow::Borrowed))
    }

    fn get_vacant_object_hash(&mut self) -> Result<VacantObjectHash, DBError> {
        self.get_vacant_entry_hash()
    }

    fn clear_objects(&mut self) -> Result<(), DBError> {
        // `InMemory` has its own garbage collection
        Ok(())
    }

    fn memory_usage(&self) -> RepositoryMemoryUsage {
        let mut mem = self.hashes.get_memory_usage();
        mem.nshapes = self.shapes.nshapes();
        mem
    }

    fn get_shape(&self, shape_id: DirectoryShapeId) -> Result<ShapeStrings, DBError> {
        self.shapes
            .get_shape(shape_id)
            .map(ShapeStrings::SliceIds)
            .map_err(Into::into)
    }

    fn make_shape(
        &mut self,
        dir: &[(StringId, DirEntryId)],
    ) -> Result<Option<DirectoryShapeId>, DBError> {
        self.shapes.make_shape(dir).map_err(Into::into)
    }

    fn synchronize_strings(&mut self, string_interner: &StringInterner) -> Result<(), DBError> {
        self.string_interner.extend_from(string_interner);

        Ok(())
    }

    fn get_str(&self, string_id: StringId) -> Option<&str> {
        self.string_interner.get(string_id)
    }

    fn get_current_offset(&self) -> Result<u64, DBError> {
        unimplemented!()
    }

    fn append_serialized_data(&mut self, data: &[u8]) -> Result<(), DBError> {
        unimplemented!()
    }
    fn synchronize_full(&mut self) -> Result<(), DBError> {
        unimplemented!()
    }
    fn get_value_from_offset(&self, buffer: &mut Vec<u8>, offset: u64) -> Result<(), DBError> {
        unimplemented!()
    }
}

impl InMemory {
    pub fn try_new() -> Result<Self, std::io::Error> {
        // TODO - TE-210: Remove once we hace proper support for history modes.
        let garbage_collector_disabled = std::env::var("DISABLE_INMEM_CONTEXT_GC")
            .unwrap_or_else(|_| "false".to_string())
            .parse::<bool>()
            .expect("Provided `DISABLE_INMEM_CONTEXT_GC` value cannot be converted to bool");

        let (sender, cons, thread_handle) = if garbage_collector_disabled {
            (None, None, None)
        } else {
            let (sender, recv) = crossbeam_channel::unbounded();
            let (prod, cons) = tezos_spsc::bounded(2_000_000);

            let thread_handle = std::thread::Builder::new()
                .name("ctx-inmem-gc-thread".to_string())
                .spawn(move || {
                    GCThread {
                        cycles: Cycles::default(),
                        recv,
                        free_ids: prod,
                        pending: Vec::new(),
                    }
                    .run()
                })?;

            (Some(sender), Some(cons), Some(thread_handle))
        };

        let current_cycle = Default::default();
        let hashes = HashValueStore::new(cons);
        let context_hashes = Default::default();

        let mut context_hashes_cycles = VecDeque::with_capacity(PRESERVE_CYCLE_COUNT);
        for _ in 0..PRESERVE_CYCLE_COUNT {
            context_hashes_cycles.push_back(Default::default())
        }

        Ok(Self {
            current_cycle,
            hashes,
            sender,
            context_hashes,
            context_hashes_cycles,
            thread_handle,
            shapes: DirectoryShapes::default(),
            string_interner: StringInterner::default(),
        })
    }

    pub(crate) fn get_vacant_entry_hash(&mut self) -> Result<VacantObjectHash, DBError> {
        self.hashes.get_vacant_object_hash().map_err(Into::into)
    }

    pub(crate) fn get_hash(&self, hash_id: HashId) -> Result<Option<&ObjectHash>, DBError> {
        self.hashes.get_hash(hash_id).map_err(Into::into)
    }

    pub(crate) fn get_value(&self, hash_id: HashId) -> Result<Option<&[u8]>, DBError> {
        self.hashes.get_value(hash_id).map_err(Into::into)
    }

    fn contains(&self, hash_id: HashId) -> Result<bool, DBError> {
        self.hashes.contains(hash_id).map_err(Into::into)
    }

    pub fn write_batch(&mut self, batch: Vec<(HashId, Arc<[u8]>)>) -> Result<(), DBError> {
        for (hash_id, value) in batch {
            self.hashes.insert_value_at(hash_id, Arc::clone(&value))?;
            self.current_cycle.insert(hash_id, Some(value));
        }
        Ok(())
    }

    pub fn new_cycle_started(&mut self) {
        if let Some(sender) = &self.sender {
            let values_in_cycle = std::mem::take(&mut self.current_cycle);
            let new_ids = self.hashes.take_new_ids();

            if let Err(e) = sender.try_send(Command::StartNewCycle {
                values_in_cycle,
                new_ids,
            }) {
                eprintln!("Fail to send Command::StartNewCycle to GC worker: {:?}", e);
            }

            if let Some(unused) = self.context_hashes_cycles.pop_front() {
                for hash in unused {
                    self.context_hashes.remove(&hash);
                }
            }
            self.context_hashes_cycles.push_back(Default::default());
        }
    }

    pub fn block_applied(&mut self, reused: Vec<HashId>) {
        if let Some(sender) = &self.sender {
            if let Err(e) = sender.send(Command::MarkReused { reused }) {
                eprintln!("Fail to send Command::MarkReused to GC worker: {:?}", e);
            }
        }
    }

    pub fn get_context_hash_impl(&self, context_hash: &ContextHash) -> Option<(HashId, u64)> {
        let mut hasher = DefaultHasher::new();
        hasher.write(context_hash.as_ref());
        let hashed = hasher.finish();

        self.context_hashes.get(&hashed).cloned().map(|v| (v, 0))
    }

    pub fn put_context_hash_impl(&mut self, commit_hash_id: HashId) -> Result<(), DBError> {
        let commit_hash = self
            .hashes
            .get_hash(commit_hash_id)?
            .ok_or(DBError::MissingObject {
                hash_id: commit_hash_id,
            })?;

        let mut hasher = DefaultHasher::new();
        hasher.write(&commit_hash[..]);
        let hashed = hasher.finish();

        self.context_hashes.insert(hashed, commit_hash_id);
        if let Some(back) = self.context_hashes_cycles.back_mut() {
            back.push(hashed);
        };

        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn put_object_hash(&mut self, entry_hash: ObjectHash) -> HashId {
        let vacant = self.get_vacant_entry_hash().unwrap();
        vacant.write_with(|entry| *entry = entry_hash)
    }
}

impl Drop for InMemory {
    fn drop(&mut self) {
        let sender = match self.sender.take() {
            Some(sender) => sender,
            None => return,
        };

        if let Err(e) = sender.send(Command::Close) {
            eprintln!("Fail to send Command::Close to GC worker: {:?}", e);
            return;
        }

        let thread_handle = match self.thread_handle.take() {
            Some(thread_handle) => thread_handle,
            None => return,
        };

        if let Err(e) = thread_handle.join() {
            eprintln!("Fail to join  GC worker thread: {:?}", e);
        }
    }
}
