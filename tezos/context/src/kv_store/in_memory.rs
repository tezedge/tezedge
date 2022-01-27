// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Implementation of an in-memory repository.

use std::{
    borrow::Cow,
    collections::{hash_map::DefaultHasher, BTreeMap, VecDeque},
    hash::Hasher,
    mem::size_of,
    rc::Rc,
    sync::{atomic::Ordering, Arc, RwLock},
    thread::JoinHandle,
};

#[cfg(test)]
use crate::serialize::persistent::AbsoluteOffset;

use crossbeam_channel::Sender;
use crypto::hash::ContextHash;
use tezos_timing::{RepositoryMemoryUsage, SerializeStats};

use crate::{
    gc::{
        worker::{Command, Cycles, GCThread, GC_PENDING_HASHIDS, PRESERVE_CYCLE_COUNT},
        GarbageCollectionError, GarbageCollector,
    },
    hash::ObjectHash,
    persistent::{DBError, Flushable, KeyValueStoreBackend, Persistable, ReadStatistics},
    working_tree::{
        shape::{DirectoryShapeId, DirectoryShapes, ShapeStrings},
        storage::{DirEntryId, DirectoryOrInodeId, Storage},
        string_interner::{StringId, StringInterner},
        working_tree::{PostCommitData, WorkingTree},
        Commit, Object, ObjectReference,
    },
    ContextKeyValueStore, IndexApi, Map, Persistent, TezedgeIndex,
};
use crate::{persistent::get_commit_hash, serialize::in_memory};

use tezos_spsc::Consumer;

use super::{index_map::IndexMap, persistent::PersistentConfiguration, HashIdError};
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
            hashes: IndexMap::with_chunk_capacity(10_000_000), // ~320MB
            values: IndexMap::with_chunk_capacity(10_000_000), // ~80MB
            free_ids: consumer.into(),
            new_ids: Vec::with_capacity(1024), // ~8KB
            values_bytes: 0,
        } // Total ~400MB
    }

    pub fn get_memory_usage(
        &self,
        strings_total_bytes: usize,
        shapes_total_bytes: usize,
        commit_index_total_bytes: usize,
        nshapes: usize,
    ) -> RepositoryMemoryUsage {
        let values_bytes = self.values_bytes;
        let values_capacity = self.values.capacity();
        let hashes_capacity = self.hashes.capacity();

        let total_bytes = values_bytes
            .saturating_add(values_capacity * size_of::<Option<Arc<[u8]>>>())
            .saturating_add(values_capacity * 16) // Each `Arc` has 16 extra bytes for the counters
            .saturating_add(hashes_capacity * size_of::<ObjectHash>())
            .saturating_add(strings_total_bytes)
            .saturating_add(shapes_total_bytes)
            .saturating_add(commit_index_total_bytes);

        RepositoryMemoryUsage {
            values_bytes,
            values_capacity,
            values_length: self.values.len(),
            hashes_capacity,
            hashes_length: self.hashes.len(),
            total_bytes,
            npending_free_ids: self.free_ids.as_ref().map(|c| c.len()).unwrap_or(0),
            gc_npending_free_ids: GC_PENDING_HASHIDS.load(Ordering::Acquire),
            nshapes,
            strings_total_bytes,
            shapes_total_bytes,
            commit_index_total_bytes,
            new_ids_cap: self.new_ids.capacity(),
        }
    }

    pub(crate) fn clear(&mut self) {
        *self = Self {
            hashes: IndexMap::empty(),
            values: IndexMap::empty(),
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

        Ok(VacantObjectHash {
            entry: Some(entry),
            hash_id,
        })
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
        let new_ids = std::mem::take(&mut self.new_ids);
        self.new_ids = Vec::with_capacity(128 * 1024);
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
    fn reload_database(&mut self) -> Result<(), DBError> {
        self.reload_database();
        Ok(())
    }

    fn contains(&self, hash_id: HashId) -> Result<bool, DBError> {
        self.contains(hash_id)
    }

    fn put_context_hash(&mut self, object_ref: ObjectReference) -> Result<(), DBError> {
        self.put_context_hash_impl(object_ref.hash_id())
    }

    fn get_context_hash(
        &self,
        context_hash: &ContextHash,
    ) -> Result<Option<ObjectReference>, DBError> {
        Ok(self.get_context_hash_impl(context_hash).map(Into::into))
    }

    fn get_hash(&self, object_ref: ObjectReference) -> Result<Cow<ObjectHash>, DBError> {
        self.get_hash(object_ref.hash_id()).map(Cow::Borrowed)
    }

    fn get_vacant_object_hash(&mut self) -> Result<VacantObjectHash, DBError> {
        self.get_vacant_entry_hash()
    }

    fn memory_usage(&self) -> RepositoryMemoryUsage {
        let strings_total_bytes = self.string_interner.memory_usage().total_bytes;
        let shapes_total_bytes = self.shapes.total_bytes();
        let commit_index_total_bytes = self.context_hashes.capacity()
            * (std::mem::size_of::<HashId>() + std::mem::size_of::<u64>());

        self.hashes.get_memory_usage(
            strings_total_bytes,
            shapes_total_bytes,
            commit_index_total_bytes,
            self.shapes.nshapes(),
        )
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

    fn synchronize_strings_from(&mut self, string_interner: &StringInterner) {
        self.string_interner.extend_from(string_interner);
    }

    fn get_str(&self, string_id: StringId) -> Option<Cow<str>> {
        self.string_interner.get_str(string_id).ok()
    }

    fn get_object(
        &self,
        object_ref: ObjectReference,
        storage: &mut Storage,
        strings: &mut StringInterner,
    ) -> Result<Object, DBError> {
        let object_bytes = self.get_value(object_ref.hash_id())?.unwrap_or(&[]);
        in_memory::deserialize_object(object_bytes, storage, strings, self).map_err(Into::into)
    }

    fn get_inode(
        &self,
        object_ref: ObjectReference,
        storage: &mut Storage,
        strings: &mut StringInterner,
    ) -> Result<DirectoryOrInodeId, DBError> {
        let object_bytes = self.get_value(object_ref.hash_id())?.unwrap_or(&[]);
        in_memory::deserialize_inode(object_bytes, storage, strings, self).map_err(Into::into)
    }

    fn get_object_bytes<'a>(
        &self,
        object_ref: ObjectReference,
        buffer: &'a mut Vec<u8>,
    ) -> Result<&'a [u8], DBError> {
        let slice = self.get_value(object_ref.hash_id())?.unwrap_or(&[]);

        buffer.clear();
        buffer.extend_from_slice(slice);

        Ok(buffer)
    }

    fn commit(
        &mut self,
        working_tree: &WorkingTree,
        parent_commit_ref: Option<ObjectReference>,
        author: String,
        message: String,
        date: u64,
    ) -> Result<(ContextHash, Box<SerializeStats>), DBError> {
        let PostCommitData {
            commit_ref,
            batch,
            reused,
            serialize_stats,
            ..
        } = working_tree
            .prepare_commit(
                date,
                author,
                message,
                parent_commit_ref,
                self,
                Some(in_memory::serialize_object),
                None,
                true,
                false,
            )
            .map_err(Box::new)?;

        self.write_batch(batch)?;
        self.put_context_hash(commit_ref)?;
        self.block_applied(reused);

        let commit_hash = get_commit_hash(commit_ref, self).map_err(Box::new)?;
        Ok((commit_hash, serialize_stats))
    }

    fn get_hash_id(&self, object_ref: ObjectReference) -> Result<HashId, DBError> {
        object_ref.hash_id_opt().ok_or(DBError::HashIdFailed)
    }

    fn take_strings_on_reload(&mut self) -> Option<StringInterner> {
        // On reload, `Self::string_interner` contains all strings and their hashes
        let string_interner = std::mem::take(&mut self.string_interner);

        // In the repository, we only want strings without their hashes
        self.synchronize_strings_from(&string_interner);

        self.string_interner
            .set_to_serialize_index(string_interner.get_to_serialize_index());

        Some(string_interner)
    }

    fn make_hash_id_ready_for_commit(&mut self, hash_id: HashId) -> Result<HashId, DBError> {
        // Unused HashId are garbage collected
        Ok(hash_id)
    }

    fn get_read_statistics(&self) -> Result<Option<ReadStatistics>, DBError> {
        Ok(None)
    }

    #[cfg(test)]
    fn synchronize_data(
        &mut self,
        batch: &[(HashId, Arc<[u8]>)],
        _output: &[u8],
    ) -> Result<Option<AbsoluteOffset>, DBError> {
        self.write_batch(batch.to_vec())?;
        Ok(None)
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

    fn reload_database(&mut self) {
        let (tree, parent_hash, commit) = {
            let mut ondisk = Persistent::try_new(PersistentConfiguration {
                db_path: Some("/tmp/tezedge/context".to_string()),
                // db_path: Some("/home/sebastien/tmp/replay/context".to_string()),
                startup_check: true,
                read_mode: true,
            })
            .unwrap();

            ondisk.reload_database().unwrap();

            // let checkout_context_hash = ContextHash::from_base58_check("CoW9AT5QuvSm3zYnxGdWgM56f1QyfF4miyNezsHveSsWap2WVqjL").unwrap();
            let checkout_context_hash: ContextHash = ondisk.get_last_context_hash().unwrap();

            println!("CHECKOUT {:?}", checkout_context_hash);

            let read_repo: Arc<RwLock<ContextKeyValueStore>> = Arc::new(RwLock::new(ondisk));
            let index = TezedgeIndex::new(Arc::clone(&read_repo), None);
            let context = index.checkout(&checkout_context_hash).unwrap().unwrap();

            // Take the commit from repository
            let commit: Commit = index
                .fetch_commit_from_context_hash(&checkout_context_hash)
                .unwrap()
                .unwrap();

            // If the commit has a parent, fetch it
            // It is necessary for the snapshot to have it in its db
            let parent_hash: Option<ObjectHash> = match commit.parent_commit_ref {
                Some(parent) => {
                    let repo = read_repo.read().unwrap();
                    Some(repo.get_hash(parent).unwrap().into_owned())
                }
                None => None,
            };

            // Traverse the tree, to store it in the `Storage`
            context.tree.traverse_working_tree(false).unwrap();

            context.index.storage.borrow_mut().forget_references();

            // Extract the `Storage`, `StringInterner` and `WorkingTree` from
            // the index
            (
                Rc::try_unwrap(context.tree).ok().unwrap(),
                // context.index.storage.take(),
                // context.index.string_interner.take().unwrap(),
                parent_hash,
                commit,
            )
        };

        {
            // Put the parent hash in the new repository
            let parent_ref: Option<ObjectReference> =
                parent_hash.map(|parent_hash| self.put_hash(parent_hash).unwrap().into());

            let commit = self
                .commit(
                    &tree,
                    parent_ref,
                    commit.author,
                    commit.message,
                    commit.time,
                )
                .unwrap();

            println!("COMMIT {:?}", commit);

            self.string_interner = tree.index.string_interner.take().unwrap();
        }
    }

    pub fn put_hash(&mut self, hash: ObjectHash) -> Result<HashId, DBError> {
        let hash_id = self
            .get_vacant_object_hash()?
            .write_with(|entry| *entry = hash);
        Ok(hash_id)
    }

    pub(crate) fn get_vacant_entry_hash(&mut self) -> Result<VacantObjectHash, DBError> {
        self.hashes.get_vacant_object_hash().map_err(Into::into)
    }

    pub(crate) fn get_hash(&self, hash_id: HashId) -> Result<&ObjectHash, DBError> {
        self.hashes
            .get_hash(hash_id)?
            .ok_or_else(|| DBError::HashNotFound {
                object_ref: hash_id.into(),
            })
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

    pub fn get_context_hash_impl(&self, context_hash: &ContextHash) -> Option<HashId> {
        let mut hasher = DefaultHasher::new();
        hasher.write(context_hash.as_ref());
        let hashed = hasher.finish();

        self.context_hashes.get(&hashed).cloned()
    }

    pub fn put_context_hash_impl(&mut self, commit_hash_id: HashId) -> Result<(), DBError> {
        let commit_hash = self
            .hashes
            .get_hash(commit_hash_id)?
            .ok_or(DBError::MissingObject {
                object_ref: commit_hash_id.into(),
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
