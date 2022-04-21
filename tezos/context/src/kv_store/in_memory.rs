// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Implementation of an in-memory repository.

use std::{
    borrow::Cow,
    collections::VecDeque,
    mem::size_of,
    rc::Rc,
    sync::{atomic::Ordering, Arc},
    thread::JoinHandle,
};

#[cfg(test)]
use crate::serialize::persistent::AbsoluteOffset;

use crossbeam_channel::Sender;
use crypto::hash::ContextHash;

use parking_lot::RwLock;
use tezos_timing::{RepositoryMemoryUsage, SerializeStats};

use crate::{
    chunks::{ChunkedVec, SharedIndexMap},
    gc::{
        jemalloc::debug_jemalloc,
        worker::{
            Command, GCThread, GC_PENDING_HASHIDS, NEW_IDS_CHUNK_CAPACITY, PRESERVE_BLOCK_LEVEL,
        },
        GarbageCollectionError, GarbageCollector,
    },
    hash::ObjectHash,
    persistent::{
        DBError, Flushable, KeyValueStoreBackend, Persistable, ReadStatistics, ReloadError,
    },
    working_tree::{
        shape::{DirectoryShapeId, DirectoryShapes, ShapeStrings},
        storage::{DirEntryId, DirectoryOrInodeId, Storage},
        string_interner::{StringId, StringInterner},
        working_tree::{PostCommitData, SerializeOutput, WorkingTree},
        Commit, Object, ObjectReference,
    },
    ContextKeyValueStore, IndexApi, Map, Persistent, TezedgeIndex,
};
use crate::{persistent::get_commit_hash, serialize::in_memory};

use tezos_spsc::Consumer;

use super::{
    inline_boxed_slice::InlinedBoxedSlice, persistent::PersistentConfiguration, HashIdError,
};
use super::{HashId, VacantObjectHash};

pub const BATCH_CHUNK_CAPACITY: usize = 8 * 1024;

#[derive(Clone, Debug)]
pub struct InMemoryConfiguration {
    pub db_path: Option<String>,
    pub startup_check: bool,
}

#[derive(Debug)]
pub struct HashObjectStore {
    hashes: SharedIndexMap<HashId, Option<Box<ObjectHash>>, OBJECTS_CHUNK_CAPACITY>,
    objects: SharedIndexMap<HashId, Option<InlinedBoxedSlice>, OBJECTS_CHUNK_CAPACITY>,
    free_ids: Option<Consumer<HashId>>,
    new_ids: ChunkedVec<HashId, NEW_IDS_CHUNK_CAPACITY>,
    values_bytes: usize,
}

pub const OBJECTS_CHUNK_CAPACITY: usize = 1_000;
pub const NEW_IDS_LIMIT: usize = 20_000;

impl HashObjectStore {
    pub(crate) fn new<T>(consumer: T) -> Self
    where
        T: Into<Option<Consumer<HashId>>>,
    {
        Self {
            hashes: SharedIndexMap::default(),  // 32KB
            objects: SharedIndexMap::default(), // 16KB
            free_ids: consumer.into(),
            new_ids: ChunkedVec::default(), // ~120KB
            values_bytes: 0,
        } // Total ~168KB
    }

    pub fn get_memory_usage(
        &self,
        strings_total_bytes: usize,
        shapes_total_bytes: usize,
        nshapes: usize,
    ) -> RepositoryMemoryUsage {
        let values_bytes = self.values_bytes;
        let objects_capacity = self.objects.capacity();
        let hashes_capacity = self.hashes.capacity();

        let total_bytes = values_bytes
            .saturating_add(objects_capacity * size_of::<Option<Box<[u8]>>>())
            .saturating_add(hashes_capacity * size_of::<Option<Box<ObjectHash>>>())
            .saturating_add(strings_total_bytes)
            .saturating_add(shapes_total_bytes);

        RepositoryMemoryUsage {
            values_bytes,
            values_capacity: objects_capacity,
            values_length: self.objects.len(),
            hashes_capacity,
            hashes_length: self.hashes.len(),
            total_bytes,
            npending_free_ids: self.free_ids.as_ref().map(|c| c.len()).unwrap_or(0),
            gc_npending_free_ids: GC_PENDING_HASHIDS.load(Ordering::Acquire),
            nshapes,
            strings_total_bytes,
            shapes_total_bytes,
            commit_index_total_bytes: 0,
            new_ids_cap: self.new_ids.capacity(),
        }
    }

    pub(crate) fn clear(&mut self) {
        *self = Self {
            hashes: SharedIndexMap::empty(),
            objects: SharedIndexMap::empty(),
            free_ids: self.free_ids.take(),
            new_ids: ChunkedVec::empty(),
            values_bytes: 0,
        }
    }

    pub(crate) fn get_vacant_object_hash(&mut self) -> Result<VacantObjectHash, HashIdError> {
        let vacant = match self.get_free_id() {
            Some(free_hash_id) => {
                self.new_ids.push(free_hash_id);
                VacantObjectHash::new_existing_id(&mut self.hashes, free_hash_id)
            }
            None => VacantObjectHash::new_push(&mut self.hashes, &mut self.new_ids),
        };

        Ok(vacant)
    }

    fn get_free_id(&mut self) -> Option<HashId> {
        self.free_ids.as_mut()?.pop().ok()
    }

    pub(crate) fn insert_object_at(
        &mut self,
        hash_id: HashId,
        value: InlinedBoxedSlice,
    ) -> Result<(), HashIdError> {
        self.objects.insert_at(hash_id, value)
    }

    pub(crate) fn get_hash(&self, hash_id: HashId) -> Option<ObjectHash> {
        self.hashes
            .with(hash_id, |hash| match hash {
                Some(Some(hash)) => Some(**hash),
                _ => None,
            })
            .ok()
            .flatten()
    }

    pub(crate) fn with_value<F, R>(&self, hash_id: HashId, fun: F) -> Result<R, DBError>
    where
        F: FnOnce(Option<&Option<InlinedBoxedSlice>>) -> R,
    {
        Ok(self.objects.with(hash_id, fun)?)
    }

    pub(crate) fn contains(&self, hash_id: HashId) -> Result<bool, HashIdError> {
        self.objects.with(hash_id, |v| matches!(v, Some(Some(_))))
    }

    fn take_new_ids(&mut self) -> ChunkedVec<HashId, NEW_IDS_CHUNK_CAPACITY> {
        std::mem::take(&mut self.new_ids)
    }
}

#[derive(Debug)]
struct ContextHashes {
    /// Map `ContextHash` to its `HashId`
    ///
    /// Used on checkouts and commits
    context_hashes: Map<ContextHash, HashId>,
    /// Highest block level
    ///
    /// Used to know when to drop context hashes
    highest_block_level: u32,
    /// List of `ContextHash` by block level
    ///
    /// When the block level increase, we drop the context hashes created
    /// at the `block_level - 3`
    context_hashes_by_level: VecDeque<Vec<ContextHash>>,
}

impl Default for ContextHashes {
    fn default() -> Self {
        let mut context_hashes_by_level = VecDeque::new();
        for _ in 0..=PRESERVE_BLOCK_LEVEL + 1 {
            context_hashes_by_level.push_back(Vec::default());
        }

        Self {
            context_hashes: Default::default(),
            context_hashes_by_level,
            highest_block_level: 0,
        }
    }
}

impl ContextHashes {
    fn pop_context_hashes(&mut self) {
        let hashes = match self.context_hashes_by_level.pop_front() {
            Some(hashes) if !hashes.is_empty() => hashes,
            _ => return,
        };

        for hash in &hashes {
            self.context_hashes.remove(hash);
        }
    }

    fn get(&self, context_hash: &ContextHash) -> Option<HashId> {
        self.context_hashes.get(context_hash).copied()
    }

    fn set_level(&mut self, block_level: u32) {
        if block_level > self.highest_block_level {
            self.pop_context_hashes();
            self.context_hashes_by_level.push_back(Default::default());
            self.highest_block_level = block_level;
        }
    }

    fn insert(&mut self, context_hash: ContextHash, context_hash_id: HashId) {
        self.context_hashes
            .insert(context_hash.clone(), context_hash_id);

        let last = match self.context_hashes_by_level.back_mut() {
            Some(last) => last,
            None => return,
        };

        last.push(context_hash);
    }
}

pub struct InMemory {
    hashes_objects: HashObjectStore,
    sender: Option<Sender<Command>>,
    context_hashes: ContextHashes,
    thread_handle: Option<JoinHandle<()>>,
    shapes: DirectoryShapes,
    string_interner: StringInterner,
    configuration: InMemoryConfiguration,
    self_ptr: Option<Arc<RwLock<ContextKeyValueStore>>>,
    latest_context_hash: Option<ContextHash>,
}

impl GarbageCollector for InMemory {
    fn new_cycle_started(&mut self) -> Result<(), GarbageCollectionError> {
        Ok(())
    }

    fn block_applied(
        &mut self,
        block_level: u32,
        context_hash: &ContextHash,
    ) -> Result<(), GarbageCollectionError> {
        let context_hash_id = match self
            .get_context_hash(context_hash)?
            .and_then(|c| c.hash_id_opt())
        {
            Some(context_hash_id) => context_hash_id,
            None => {
                return Err(GarbageCollectionError::ContextHashNotFound {
                    context_hash: context_hash.clone(),
                })
            }
        };
        self.context_hashes.set_level(block_level);
        self.block_applied(block_level, context_hash_id);

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
    fn reload_database(&mut self) -> Result<(), ReloadError> {
        self.reload_database()
    }

    fn store_own_repository(&mut self, repository: Arc<RwLock<ContextKeyValueStore>>) {
        self.self_ptr.replace(repository.clone());
        if let Some(sender) = self.sender.as_ref() {
            sender
                .send(Command::StoreRepository { repository })
                .unwrap();
        };
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
        self.get_hash(object_ref.hash_id()).map(Cow::Owned)
    }

    fn get_vacant_object_hash(&mut self) -> Result<VacantObjectHash, DBError> {
        self.get_vacant_entry_hash()
    }

    fn memory_usage(&self) -> RepositoryMemoryUsage {
        let strings_total_bytes = self.string_interner.memory_usage().total_bytes;
        let shapes_total_bytes = self.shapes.total_bytes();

        self.hashes_objects.get_memory_usage(
            strings_total_bytes,
            shapes_total_bytes,
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
        self.with_value(object_ref.hash_id(), |value| {
            let object_bytes = match value {
                Some(Some(value)) => value,
                _ => return Err(DBError::MissingObject { object_ref }),
            };
            in_memory::deserialize_object(object_bytes, storage, strings, self).map_err(Into::into)
        })?
    }

    fn get_inode(
        &self,
        object_ref: ObjectReference,
        storage: &mut Storage,
        strings: &mut StringInterner,
    ) -> Result<DirectoryOrInodeId, DBError> {
        self.with_value(object_ref.hash_id(), |value| {
            let object_bytes = match value {
                Some(Some(value)) => value,
                _ => return Err(DBError::MissingObject { object_ref }),
            };
            in_memory::deserialize_inode(object_bytes, storage, strings, self).map_err(Into::into)
        })?
    }

    fn get_object_bytes<'a>(
        &self,
        object_ref: ObjectReference,
        buffer: &'a mut Vec<u8>,
    ) -> Result<&'a [u8], DBError> {
        buffer.clear();

        self.with_value(object_ref.hash_id(), |value| {
            if let Some(Some(value)) = value {
                buffer.extend_from_slice(value)
            };
        })?;

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
        self.commit_impl(working_tree, parent_commit_ref, author, message, date)
    }

    fn add_serialized_objects(
        &mut self,
        batch: ChunkedVec<(HashId, InlinedBoxedSlice), { BATCH_CHUNK_CAPACITY }>,
        _output: &mut SerializeOutput,
    ) -> Result<(), DBError> {
        self.write_batch(batch)
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

    fn latest_context_hashes(&self, _count: i64) -> Result<Vec<ContextHash>, DBError> {
        // Ignore `count`, the in-memory context only has at most 1 context hash on {re}start
        if let Some(latest) = self.latest_context_hash.clone() {
            Ok(vec![latest])
        } else {
            Ok(vec![])
        }
    }

    #[cfg(test)]
    fn synchronize_data(
        &mut self,
        batch: &[(HashId, InlinedBoxedSlice)],
        _output: &[u8],
    ) -> Result<Option<AbsoluteOffset>, DBError> {
        let mut vec = ChunkedVec::<_, BATCH_CHUNK_CAPACITY>::default();
        for item in batch {
            vec.push(item.clone());
        }
        self.write_batch(vec)?;
        Ok(None)
    }
}

impl InMemory {
    pub fn try_new(configuration: InMemoryConfiguration) -> Result<Self, std::io::Error> {
        log!("Opening in-memory context {:?}", configuration);

        // TODO - TE-210: Remove once we hace proper support for history modes.
        let garbage_collector_disabled = std::env::var("DISABLE_INMEM_CONTEXT_GC")
            .unwrap_or_else(|_| "false".to_string())
            .parse::<bool>()
            .expect("Provided `DISABLE_INMEM_CONTEXT_GC` value cannot be converted to bool");

        let (sender, thread_handle, hashes) = if garbage_collector_disabled {
            (None, None, HashObjectStore::new(None))
        } else {
            let (sender, recv) = crossbeam_channel::unbounded();
            let (producer, consumer) = tezos_spsc::bounded(2_000_000);
            let hashes = HashObjectStore::new(consumer);
            let objects_view = hashes.objects.get_view();
            let hashes_view = hashes.hashes.get_view();
            let conf = configuration.clone();

            let thread_handle = std::thread::Builder::new()
                .name("ctx-inmem-gc-thread".to_string())
                .spawn(move || {
                    GCThread::new(recv, producer, objects_view, hashes_view, conf).run()
                })?;

            (Some(sender), Some(thread_handle), hashes)
        };

        Ok(Self {
            hashes_objects: hashes,
            sender,
            context_hashes: ContextHashes::default(),
            thread_handle,
            shapes: DirectoryShapes::default(),
            string_interner: StringInterner::default(),
            configuration,
            self_ptr: None,
            latest_context_hash: None,
        })
    }

    /// Reload context from disk
    fn reload_database(&mut self) -> Result<(), ReloadError> {
        debug_jemalloc();

        let (tree, parent_hash, commit) = {
            let base_path = self
                .configuration
                .db_path
                .clone()
                .unwrap_or_else(|| "/tmp/tezedge".to_string());

            let mut ondisk = Persistent::try_new(PersistentConfiguration {
                db_path: Some(base_path),
                startup_check: self.configuration.startup_check,
                read_mode: true,
            })?;

            ondisk.reload_database()?;

            let checkout_context_hash: ContextHash = ondisk
                .get_last_context_hash()
                .ok_or(ReloadError::LastCommitNotFound)?;

            let read_repo: Arc<RwLock<ContextKeyValueStore>> = Arc::new(RwLock::new(ondisk));
            let index = TezedgeIndex::new(Arc::clone(&read_repo), None);
            let context = index
                .checkout(&checkout_context_hash)?
                .ok_or(ReloadError::CheckoutFailed)?;

            // Take the commit from repository
            let commit: Commit = index
                .fetch_commit_from_context_hash(&checkout_context_hash)?
                .ok_or(ReloadError::FetchCommitFailed)?;

            // If the commit has a parent, fetch it
            // It is necessary for our new repository to have it.
            let parent_hash: Option<ObjectHash> = match commit.parent_commit_ref {
                Some(parent) => {
                    let repo = read_repo.read();
                    Some(repo.get_hash(parent)?.into_owned())
                }
                None => None,
            };

            // Traverse the tree, to store it in the `Storage`
            context.tree.traverse_working_tree(false)?;

            // Forget HashId and offsets, they will be recomputed.
            context.index.storage.borrow_mut().forget_references();

            // Extract the `Storage`, `StringInterner` and `WorkingTree` from
            // the index
            (
                Rc::try_unwrap(context.tree).ok().unwrap(), // Never fail, there is 1 reference alive
                parent_hash,
                commit,
            )
        };

        // Put the parent hash in the new repository (in-memory one)
        let parent_ref: Option<ObjectReference> = match parent_hash {
            Some(parent_hash) => Some(self.put_hash(parent_hash)?.into()),
            None => None,
        };

        // Commit the tree in the in-memory repository
        let (commit_hash, _) = self
            .commit_impl(
                &tree,
                parent_ref,
                commit.author,
                commit.message,
                commit.time,
            )
            .map_err(|error| ReloadError::CommitFailed { error })?;

        self.string_interner = tree
            .index
            .string_interner
            .take()
            .ok_or(ReloadError::StringInternerNotFound)?;

        self.string_interner.shrink_to_fit();
        self.shapes.shrink_to_fit();

        log!("[after reload] memory_usage={:#?}", self.memory_usage());
        debug_jemalloc();

        self.latest_context_hash = Some(commit_hash);

        Ok(())
    }

    fn maybe_send_new_chunks_to_gc(&mut self) {
        let sender = match self.sender.as_ref() {
            Some(sender) => sender,
            None => return,
        };

        let objects_chunks = self.hashes_objects.objects.clone_new_chunks();
        let hashes_chunks = self.hashes_objects.hashes.clone_new_chunks();

        if objects_chunks.is_none() && hashes_chunks.is_none() {
            return;
        }

        if let Err(e) = sender.send(Command::NewChunks {
            objects_chunks,
            hashes_chunks,
        }) {
            elog!("Failed to send `Command::NewChunks` to GC thread: {:?}", e);
        }
    }

    fn commit_impl(
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
                false,
            )
            .map_err(Box::new)?;

        self.write_batch(batch)?;
        self.maybe_send_new_chunks_to_gc();

        self.put_context_hash(commit_ref)?;

        let commit_hash = get_commit_hash(commit_ref, self).map_err(Box::new)?;
        Ok((commit_hash, serialize_stats))
    }

    pub fn put_hash(&mut self, hash: ObjectHash) -> Result<HashId, DBError> {
        let hash_id = self
            .get_vacant_object_hash()?
            .write_with(|entry| *entry = hash)?;
        Ok(hash_id)
    }

    pub(crate) fn get_vacant_entry_hash(&mut self) -> Result<VacantObjectHash, DBError> {
        if self.hashes_objects.new_ids.len() >= NEW_IDS_LIMIT {
            let new_ids = self.hashes_objects.take_new_ids();
            self.sender
                .as_ref()
                .map(|s| s.send(Command::MarkNewIds { new_ids }));
        }

        self.hashes_objects
            .get_vacant_object_hash()
            .map_err(Into::into)
    }

    pub(crate) fn get_hash(&self, hash_id: HashId) -> Result<ObjectHash, DBError> {
        self.hashes_objects
            .get_hash(hash_id)
            .ok_or_else(|| DBError::HashNotFound {
                object_ref: hash_id.into(),
            })
    }

    pub(crate) fn with_value<F, R>(&self, hash_id: HashId, fun: F) -> Result<R, DBError>
    where
        F: FnOnce(Option<&Option<InlinedBoxedSlice>>) -> R,
    {
        Ok(self.hashes_objects.objects.with(hash_id, fun)?)
    }

    fn contains(&self, hash_id: HashId) -> Result<bool, DBError> {
        self.hashes_objects.contains(hash_id).map_err(Into::into)
    }

    pub fn write_batch(
        &mut self,
        mut batch: ChunkedVec<(HashId, InlinedBoxedSlice), BATCH_CHUNK_CAPACITY>,
    ) -> Result<(), DBError> {
        while let Some(chunk) = batch.pop_first_chunk() {
            for (hash_id, value) in chunk.into_iter() {
                self.hashes_objects.insert_object_at(hash_id, value)?;
            }
        }
        Ok(())
    }

    pub fn block_applied(&mut self, block_level: u32, commit_hash_id: HashId) {
        let sender = match self.sender.as_ref() {
            Some(sender) => sender,
            None => return,
        };

        let new_ids = self.hashes_objects.take_new_ids();

        if let Err(e) = sender.send(Command::BlockApplied {
            new_ids,
            commit_hash_id,
            block_level,
        }) {
            eprintln!("Fail to send Command::MarkReused to GC worker: {:?}", e);
        }
    }

    pub fn get_context_hash_impl(&self, context_hash: &ContextHash) -> Option<HashId> {
        self.context_hashes.get(context_hash)
    }

    pub fn put_context_hash_impl(&mut self, context_hash_id: HashId) -> Result<(), DBError> {
        let context_hash = self
            .hashes_objects
            .get_hash(context_hash_id)
            .and_then(|h| ContextHash::try_from(&h[..]).ok())
            .ok_or(DBError::MissingObject {
                object_ref: context_hash_id.into(),
            })?;

        self.context_hashes.insert(context_hash, context_hash_id);

        Ok(())
    }
}

impl Drop for InMemory {
    fn drop(&mut self) {
        elog!("Dropping InMemory");

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
        elog!("Dropping InMemory");
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[allow(clippy::same_item_push)]
    #[test]
    fn boxed_or_inlined() {
        let mut vec = Vec::with_capacity(100);

        for i in 0..=255 {
            let boxed = InlinedBoxedSlice::from(vec.as_slice());
            assert_eq!(&*boxed, &vec);
            std::mem::drop(boxed);

            vec.push(i);
        }

        let mut vec = Vec::with_capacity(10_000);

        for _ in 0..10_000 {
            let boxed = InlinedBoxedSlice::from(vec.as_slice());
            assert_eq!(&*boxed, &vec);
            std::mem::drop(boxed);

            // Fully filled byte
            vec.push(0xFF);
        }
    }

    #[test]
    fn reload_from_disk() {
        if true {
            // Test used locally only
            return;
        }

        #[cfg(not(target_env = "msvc"))]
        use tikv_jemallocator::Jemalloc;

        #[cfg(not(target_env = "msvc"))]
        #[global_allocator]
        static GLOBAL: Jemalloc = Jemalloc;

        debug_jemalloc();

        {
            let now = std::time::Instant::now();

            let mut repo = InMemory::try_new(InMemoryConfiguration {
                db_path: None,
                startup_check: true,
            })
            .unwrap();
            repo.reload_database().unwrap();

            println!("RELOADED in {:?}", now.elapsed());
            std::thread::sleep(Duration::from_millis(10000));
            std::mem::drop(repo);
        }

        debug_jemalloc();

        println!("EVERYTHING DROPPED");
        std::thread::sleep(Duration::from_millis(20000));

        debug_jemalloc();
    }
}
