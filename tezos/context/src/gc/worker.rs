// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    num::TryFromIntError,
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use crossbeam_channel::{Receiver, RecvError};
use parking_lot::RwLock;
use static_assertions::assert_eq_size;
use tezos_timing::SerializeStats;
use thiserror::Error;

use crate::{
    chunks::{ChunkedVec, SharedChunk, SharedIndexMapError, SharedIndexMapView},
    gc::{
        jemalloc::debug_jemalloc,
        stats::{CollectorStatistics, CommitStatistics, OnMessageStatistics},
    },
    initializer::IndexInitializationError,
    kv_store::{
        in_memory::{InMemoryConfiguration, BATCH_CHUNK_CAPACITY, OBJECTS_CHUNK_CAPACITY},
        index_map::IndexMap,
        inline_boxed_slice::InlinedBoxedSlice,
        persistent::PersistentConfiguration,
        HashId, HashIdError,
    },
    persistent::{DBError, KeyValueStoreBackend},
    serialize::{
        get_object_tag,
        in_memory::{self, commit_parent_hash, iter_hash_ids},
        persistent, DeserializationError, ObjectTag, SerializationError,
    },
    working_tree::{
        storage::Storage,
        string_interner::StringInterner,
        working_tree::{MerkleError, SerializeOutput},
        Object, ObjectReference,
    },
    ContextKeyValueStore, ObjectHash, Persistent,
};

use tezos_spsc::Producer;

// Some chunks capacity
const PENDING_CHUNK_CAPACITY: usize = 100_000;
const UNUSED_CHUNK_CAPACITY: usize = 20_000;
pub const NEW_IDS_CHUNK_CAPACITY: usize = 20_000;

/// Maximum number of `HashId` we send to the main thread at once
const MAX_SEND_TO_MAIN_THREAD: usize = 200_000;

/// Maximum number of `HashId` we send to the main thread when
/// the queue was empty
const LIMIT_ON_EMPTY_CHANNEL: usize = 20_000;

/// Minimum number of block applieds before we run the
/// next garbage collection
const NAPPLIED_BEFORE_COLLECT: usize = 100;

/// Number of block level to keep, before objects/hashes
/// are garbage collected
pub const PRESERVE_BLOCK_LEVEL: u8 = 3;

/// Default interval between snapshots: 12 hours
const SNAPSHOT_DEFAULT_DELAY: Duration = Duration::from_secs(60 * 60 * 12);

/// While creating a snapshot, we write to disk some datas to avoid keeping
/// them in RAM. The following constants define when to write on disk.
const SNAPSHOT_DATA_LIMIT: usize = 20_000_000;
const SNAPSHOT_HASHES_LIMIT: usize = 625_000;
const SNAPSHOT_STRINGS_LIMIT: usize = 10_000_000;

/// Used for statistics
///
/// Number of items in `GCThread::pending`.
pub(crate) static GC_PENDING_HASHIDS: AtomicUsize = AtomicUsize::new(0);

type ChunkIndex = u32;

/// A number in the range [0; 127]
///
/// This replace an `Option<u8>` so that `Generation` is 1 byte
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct Generation {
    inner: u8,
}

impl Default for Generation {
    fn default() -> Self {
        Self::zero()
    }
}

#[cfg(test)]
impl From<u8> for Generation {
    fn from(inner: u8) -> Self {
        assert!(inner < 128);
        Self { inner }
    }
}

impl Generation {
    const fn none() -> Self {
        Self { inner: 1 << 7 }
    }

    #[cfg(test)]
    const fn is_none(self) -> bool {
        self.inner >> 7 != 0
    }

    const fn zero() -> Self {
        Self { inner: 0 }
    }

    const fn wrapping_sub(self, rhs: u8) -> Self {
        if rhs > self.inner {
            assert!(rhs < 128); // Do not handle this case

            let diff = rhs - self.inner;
            Self { inner: 128 - diff }
        } else {
            Self {
                inner: self.inner - rhs,
            }
        }
    }

    const fn wrapping_increment(&self) -> Self {
        if self.inner == 127 {
            Self { inner: 0 }
        } else {
            Self {
                inner: self.inner + 1,
            }
        }
    }
}

pub(crate) struct GCThread {
    /// Queue of `HashId` the main thread can reuse
    free_ids: Producer<HashId>,
    /// Channel to receive `Command` from the main thead
    recv: Receiver<Command>,

    /// List of `HashId` unused and ready to be send to the main thread
    /// for reuse
    pending: ChunkedVec<HashId, PENDING_CHUNK_CAPACITY>,
    /// Debug mode, can be set with the environment variable
    /// `TEZEDGE_GC_DEBUG`
    debug: bool,
    /// Number of `HashId` used in this block
    ///
    /// Used for logging
    new_ids_in_commit: usize,
    /// Timestamp of the last garbage collection
    ///
    /// Used for logging
    last_gc_timestamp: Option<Instant>,

    /// The highest block level applied
    ///
    /// Used to increment, or not, `Self::current_generation`
    highest_block_level: u32,

    /// View on `InMemory::objects`
    ///
    /// Objects are read and deallocated from this field
    objects_view: SharedIndexMapView<HashId, Option<InlinedBoxedSlice>, OBJECTS_CHUNK_CAPACITY>,
    /// View on `InMemory::hashes`
    ///
    /// Hashes are deallocated from this field
    hashes_view: SharedIndexMapView<HashId, Option<Box<ObjectHash>>, OBJECTS_CHUNK_CAPACITY>,

    /// The generation of each `HashId` (and their objects/hashes)
    ///
    /// When a `HashId` is used in a block, its generation is set
    /// to `Self::current_generation`.
    ///
    /// Used to know when a `HashId` can be reused
    objects_generation: IndexMap<HashId, Generation, 1_000_000>,
    /// Current generation of the garbage collector
    ///
    /// This is incremented (wrapping around) _at most_ every time the block level
    /// is incremented.
    ///
    /// Note that a block application does not necessarily increment `current_generation`
    /// More than 1 consecutive blocks may have the same `current_generation`, it might
    /// occurs in 2 cases:
    /// - A fork is created, in that case the block level is not incremented
    /// - The garbage collector is late, the main thread applied some blocks while
    ///   the garbage collection was running
    current_generation: Generation,

    /// Keep track of how many objects/hashes are alive per chunk
    ///
    /// Used to prioritize non-dead chunks when sending re-usable `HashId`
    /// to the main thread
    nalives_per_chunk: IndexMap<ChunkIndex, u32, 1_000_000>,
    /// Number of block applied since the last garbage collection.
    ///
    /// This is used to know when we can run the gc again, we run
    /// it at most every `NAPPLIED_BEFORE_COLLECT`
    napplieds_since_last_run: usize,

    in_mem_repo: Option<Arc<RwLock<ContextKeyValueStore>>>,
    configuration: InMemoryConfiguration,
    last_snapshot_timestamp: Instant,
    delay_between_snapshot: Duration,
}

assert_eq_size!([u8; 16], Option<Box<[u8]>>);
assert_eq_size!([u8; 8], Option<Box<ObjectHash>>);

#[derive(Debug, Error)]
enum GCError {
    #[error("HashId conversion failed")]
    HashIdFailed,
    #[error("Failed to traverse tree")]
    TraverseTreeError,
    #[error("Alive counter is in an invalid state")]
    InvalidAliveState,
    #[error("Int conversion failed")]
    FromIntFailed {
        #[from]
        e: TryFromIntError,
    },
    #[error("SharedIndexMap error: {error}")]
    SharedIndex {
        #[from]
        error: SharedIndexMapError,
    },
    #[error("Unable to find a frozen commit for snapshot")]
    FrozenCommitNotFound,
    #[error("DBError {error}")]
    DBError {
        #[from]
        error: DBError,
    },
    #[error("MerkleError {error}")]
    MerkleError {
        #[from]
        error: MerkleError,
    },
    #[error("Fail during deserialization {error}")]
    DeserializationError {
        #[from]
        error: DeserializationError,
    },
    #[error("Fail during serialization {error}")]
    SerializationError {
        #[from]
        error: SerializationError,
    },
    #[error("Fail while initializing repository {error}")]
    IndexInitializationError {
        #[from]
        error: IndexInitializationError,
    },
    #[error("The GC thread doesn't have access to the in-mem repository")]
    InMemContextNotFound,
    #[error("String not found in the string interner")]
    StringNotFound,
    #[error("Unable to create a valid snapshot path")]
    InvalidSnapshotPath,
    #[allow(dead_code)]
    #[error("Call to renameat2 failed with {error}")]
    RenameAt2Failed { error: std::io::Error },
}

impl From<HashIdError> for GCError {
    fn from(_: HashIdError) -> Self {
        GCError::HashIdFailed
    }
}

pub enum Command {
    MarkNewIds {
        new_ids: ChunkedVec<HashId, NEW_IDS_CHUNK_CAPACITY>,
    },
    BlockApplied {
        new_ids: ChunkedVec<HashId, NEW_IDS_CHUNK_CAPACITY>,
        commit_hash_id: HashId,
        block_level: u32,
    },
    NewChunks {
        objects_chunks: Option<Vec<SharedChunk<Option<InlinedBoxedSlice>, OBJECTS_CHUNK_CAPACITY>>>,
        hashes_chunks: Option<Vec<SharedChunk<Option<Box<ObjectHash>>, OBJECTS_CHUNK_CAPACITY>>>,
    },
    StoreRepository {
        repository: Arc<RwLock<ContextKeyValueStore>>,
    },
    Close,
}

#[cfg(not(target_os = "linux"))]
fn replace_context_with_snapshot(path_context: &str, path_snapshot: &str) -> Result<(), GCError> {
    let options = fs_extra::dir::CopyOptions {
        overwrite: true,
        skip_exist: false,
        buffer_size: 64000,
        copy_inside: true,
        content_only: true,
        depth: 0,
    };
    fs_extra::dir::move_dir(path_snapshot, path_context, &options).unwrap();
    std::fs::remove_dir_all(path_snapshot).map_err(DBError::from)?;

    Ok(())
}

// `renameat2` is a Linux syscall
#[cfg(target_os = "linux")]
fn replace_context_with_snapshot(path_context: &str, path_snapshot: &str) -> Result<(), GCError> {
    // Make sure `path_context` exist, or `renameat2` will fail
    std::fs::create_dir_all(&path_context).map_err(DBError::from)?;

    // Convert `path_{context,snapshot}` to C strings
    let cstr_snapshot =
        std::ffi::CString::new(path_snapshot).map_err(|_| GCError::InvalidSnapshotPath)?;
    let cstr_snapshot = cstr_snapshot.as_bytes_with_nul().as_ptr() as *const i8;

    let cstr_context =
        std::ffi::CString::new(path_context).map_err(|_| GCError::InvalidSnapshotPath)?;
    let cstr_context = cstr_context.as_bytes_with_nul().as_ptr() as *const i8;

    // Exchange `path_context` with `path_snapshot` atomically
    let result = unsafe {
        libc::syscall(
            libc::SYS_renameat2,
            libc::AT_FDCWD,
            cstr_snapshot,
            libc::AT_FDCWD,
            cstr_context,
            libc::RENAME_EXCHANGE,
        )
    };

    if result != 0 {
        let error = std::io::Error::last_os_error();
        return Err(GCError::RenameAt2Failed { error });
    }

    // Remove snapshot path
    std::fs::remove_dir_all(path_snapshot).map_err(DBError::from)?;

    Ok(())
}

impl GCThread {
    pub fn new(
        recv: Receiver<Command>,
        producer: Producer<HashId>,
        objects_view: SharedIndexMapView<HashId, Option<InlinedBoxedSlice>, OBJECTS_CHUNK_CAPACITY>,
        hashes_view: SharedIndexMapView<HashId, Option<Box<ObjectHash>>, OBJECTS_CHUNK_CAPACITY>,
        configuration: InMemoryConfiguration,
    ) -> Self {
        let delay_between_snapshot = match std::env::var("TEZEDGE_GC_DELAY_SNAPSHOT_SEC")
            .ok()
            .and_then(|d| d.parse().ok())
        {
            Some(delay_sec) => Duration::from_secs(delay_sec),
            _ => SNAPSHOT_DEFAULT_DELAY,
        };

        Self {
            recv,
            free_ids: producer,
            pending: ChunkedVec::default(),
            debug: false,
            objects_generation: IndexMap::default(),
            current_generation: Generation::zero(),
            objects_view,
            hashes_view,
            nalives_per_chunk: IndexMap::default(),
            new_ids_in_commit: 0,
            last_gc_timestamp: None,
            napplieds_since_last_run: 0,
            highest_block_level: 0,
            in_mem_repo: None,
            configuration,
            last_snapshot_timestamp: std::time::Instant::now(),
            delay_between_snapshot,
        }
    }

    pub(crate) fn run(mut self) {
        // Enable debug logs when `TEZEDGE_GC_DEBUG` is present
        self.debug = std::env::var("TEZEDGE_GC_DEBUG").is_ok();

        loop {
            let msg = self.recv.recv();

            self.debug(&msg);

            match msg {
                Ok(Command::NewChunks {
                    objects_chunks,
                    hashes_chunks,
                }) => {
                    self.add_chunks(objects_chunks, hashes_chunks);
                }
                Ok(Command::MarkNewIds { new_ids }) => {
                    if let Err(e) = self.mark_new_ids(new_ids) {
                        elog!("mark_new_ids failed: {:?}", e);
                    }
                    self.send_unused_on_empty_channel();
                }
                Ok(Command::BlockApplied {
                    new_ids,
                    commit_hash_id,
                    block_level,
                }) => {
                    if let Err(e) = self.handle_commit(new_ids, block_level, commit_hash_id) {
                        elog!("handle_commit failed: {:?}", e);
                    }
                }
                Ok(Command::StoreRepository { repository }) => {
                    self.in_mem_repo.replace(repository);
                }
                Ok(Command::Close) => {
                    elog!("GC received Command::Close");
                    break;
                }
                Err(e) => {
                    elog!("GC channel is closed {:?}", e);
                    break;
                }
            }
        }
        elog!("GC exited");
    }

    fn add_chunks(
        &mut self,
        objects_chunks: Option<Vec<SharedChunk<Option<InlinedBoxedSlice>, OBJECTS_CHUNK_CAPACITY>>>,
        hashes_chunks: Option<Vec<SharedChunk<Option<Box<ObjectHash>>, OBJECTS_CHUNK_CAPACITY>>>,
    ) {
        // `objects_chunks` and `hashes_chunks` are sent from the main thread
        // we append them to our `Self::objects_view` and `Self::hashes_view`
        if let Some(objects_chunks) = objects_chunks {
            self.objects_view.append_chunks(objects_chunks);
        };
        if let Some(hashes_chunks) = hashes_chunks {
            self.hashes_view.append_chunks(hashes_chunks);
        };
    }

    fn debug(&self, command: &Result<Command, RecvError>) {
        if !self.debug {
            return;
        }

        let stats = OnMessageStatistics {
            command,
            pending_command: self.recv.len(),
            pending_hash_ids_length: self.pending.len(),
            pending_hash_ids_capacity: self.pending.capacity(),
            objects_nchunks: self.objects_view.nchunks(),
            hashes_nchunks: self.hashes_view.nchunks(),
            counter_nchunks: self.objects_generation.entries.list_of_chunks.len(),
        };

        log!("{:?}", stats);
    }

    /// This method send some `HashId` without checking if they are
    /// in a dead chunk or not.
    /// We have to do this when the channel `Self::free_ids` is empty
    /// or the main thread will allocate new chunks
    fn send_unused_on_empty_channel(&mut self) {
        if self.free_ids.len() >= LIMIT_ON_EMPTY_CHANNEL {
            // `Self::free_ids` is not empty, don't send anything
            return;
        }

        let pending = match self.pending.pop_first_chunk() {
            Some(pending) if !pending.is_empty() => pending,
            _ => return,
        };

        let send_at = pending.len().saturating_sub(LIMIT_ON_EMPTY_CHANNEL);

        if let Err(e) = self.free_ids.push_slice(&pending[send_at..]) {
            elog!("GC: Fail to send free ids {:?}", e);
            self.pending.extend_from_slice(&pending);
        } else {
            self.pending.extend_from_slice(&pending[..send_at]);
        }
    }

    /// Notify the main thread that the ids are free to reused
    ///
    /// This prioritize HashId that are not in dead chunks, to avoid
    /// reallocating them.
    ///
    /// Returns the number of `HashId` sent
    fn send_unused(&mut self) -> Result<usize, GCError> {
        let sent = self.send_unused_impl()?;

        GC_PENDING_HASHIDS.store(self.pending.len(), Ordering::Release);
        Ok(sent)
    }

    /// Return `true` when the `HashId` refers to a dead chunk (the chunk is
    /// deallocated)
    fn is_in_dead_chunk(&self, hash_id: HashId) -> Result<bool, GCError> {
        let chunk_index = self.hashes_view.chunk_index_of(hash_id)? as u32;
        let is_chunk_dead = self
            .nalives_per_chunk
            .get(chunk_index)?
            .map(|n| *n == 0)
            .unwrap_or(true);

        Ok(is_chunk_dead)
    }

    fn send_unused_impl(&mut self) -> Result<usize, GCError> {
        if self.free_ids.len() > MAX_SEND_TO_MAIN_THREAD {
            // The queue is already filled enough, do not send any `HashId`
            return Ok(0);
        }

        let navailable = self.free_ids.available();
        let npending = self.pending.len();

        if npending == 0 || navailable == 0 {
            return Ok(0);
        }

        let nto_send = npending.min(navailable).min(MAX_SEND_TO_MAIN_THREAD);
        let mut to_send = Vec::<HashId>::with_capacity(nto_send);
        let mut nchunk_pending = self.pending.nchunks();

        // Iterate a first time on `Self::pending` and send `HashId` that are in
        // a non-dead chunk.
        // If after the first iteration, we didn't send enough `HashId` (`nto_send`),
        // we iterate a second time on `Self::pending` and send `HashId` that
        // are in a dead chunk too
        'outer: while let Some(chunk) = self.pending.pop_first_chunk() {
            for hash_id in chunk {
                if nchunk_pending > 0 && self.is_in_dead_chunk(hash_id)? {
                    self.pending.push(hash_id);
                } else {
                    to_send.push(hash_id);
                }

                if nchunk_pending == 0 && to_send.len() > 50_000 {
                    // `nchunk_pending` is zero, it means we already sent all `HashId` in
                    // non-dead chunks
                    break 'outer;
                }

                if to_send.len() == nto_send {
                    break 'outer;
                }
            }
            nchunk_pending = nchunk_pending.saturating_sub(1);
        }

        if let Err(e) = self.free_ids.push_slice(&to_send) {
            elog!("GC: Fail to send free ids {:?}", e);
            self.pending.extend_from_slice(&to_send);
            Ok(0)
        } else {
            Ok(to_send.len())
        }
    }

    fn for_each_child<F>(
        &self,
        hash_id: HashId,
        bytes: &mut Vec<u8>,
        fun: &mut F,
    ) -> Result<(), GCError>
    where
        F: FnMut(HashId),
    {
        bytes.clear();
        self.with_object(hash_id, |object_bytes| {
            bytes.extend_from_slice(object_bytes);
        })?;

        let tag = get_object_tag(bytes);

        match tag {
            ObjectTag::InodePointers => {
                let bytes_clone = bytes.clone();
                for hash_id in iter_hash_ids(&bytes_clone) {
                    self.for_each_child(hash_id, bytes, fun)?;
                }
            }
            _ => {
                for hash_id in iter_hash_ids(bytes) {
                    fun(hash_id);
                }
            }
        }

        Ok(())
    }

    fn with_object<F>(&self, hash_id: HashId, fun: F) -> Result<(), GCError>
    where
        F: FnOnce(&[u8]),
    {
        self.objects_view.with(hash_id, |object_bytes| {
            let object_bytes = match object_bytes {
                Some(Some(object_bytes)) => object_bytes,
                _ => {
                    elog!("Missing Object object={:?}", object_bytes);
                    return Err(GCError::TraverseTreeError);
                }
            };

            fun(object_bytes);

            Ok(())
        })?
    }

    fn traverse_and_mark_tree(
        &mut self,
        hash_id: HashId,
        hash_ids: &mut ChunkedVec<HashId, 8192>,
        nobjects: &mut usize,
        depth: usize,
        max_depth: &mut usize,
        objets_total_bytes: &mut usize,
    ) -> Result<(), GCError> {
        *nobjects += 1;
        *max_depth = depth.max(*max_depth);

        let start = hash_ids.len();
        self.with_object(hash_id, |object_bytes| {
            *objets_total_bytes += object_bytes.len();
            for hash_id in iter_hash_ids(object_bytes) {
                hash_ids.push(hash_id);
            }
        })?;
        let end = hash_ids.len();

        {
            // Update the generation of the HashId (object/hash)
            let generation = self.objects_generation.entry(hash_id)?;
            *generation = self.current_generation;
        }

        for index in start..end {
            let hash_id = hash_ids[index];
            self.traverse_and_mark_tree(
                hash_id,
                hash_ids,
                nobjects,
                depth,
                max_depth,
                objets_total_bytes,
            )?;
        }

        assert_eq!(hash_ids.len(), end);
        hash_ids.remove_last_nelems(end - start);

        Ok(())
    }

    fn copy_hash_to_snapshot(
        &self,
        hash_id: HashId,
        on_disk_repo: &mut Persistent,
        hash: &mut ObjectHash,
    ) -> Result<HashId, GCError> {
        {
            // Copy to object hash (found in the in-mem context) into `hash`
            // Lock the in-mem repo as short as possible
            let in_mem_repo = self.in_mem_repository()?.read();
            let object_hash = in_mem_repo.get_hash(ObjectReference::new(Some(hash_id), None))?;
            *hash = *object_hash;
        }

        // Copy the hash into the on-disk repository, it now has a new `HashId`
        let hash_id = on_disk_repo
            .get_vacant_object_hash()?
            .write_with(|entry| entry.copy_from_slice(hash))?;

        Ok(hash_id)
    }

    fn in_mem_repository(&self) -> Result<&Arc<RwLock<ContextKeyValueStore>>, GCError> {
        self.in_mem_repo
            .as_ref()
            .ok_or(GCError::InMemContextNotFound)
    }

    /// Write data to disk when some limits have been reached
    ///
    /// This avoid keeping everything in memory before the snapshot
    /// is fully completed.
    fn maybe_write_to_disk(
        &self,
        output: &mut SerializeOutput,
        on_disk_repo: &mut Persistent,
    ) -> Result<(), GCError> {
        let data_len = output.len();
        let hashes_len = on_disk_repo.hashes_in_memory_len();
        let strings_len = on_disk_repo
            .string_interner
            .new_bytes_since_last_serialize();

        let data_limit_reached = data_len >= SNAPSHOT_DATA_LIMIT;
        let hashes_limit_reached = hashes_len >= SNAPSHOT_HASHES_LIMIT;
        let strings_limit_reached = strings_len >= SNAPSHOT_STRINGS_LIMIT;

        if data_limit_reached || hashes_limit_reached || strings_limit_reached {
            on_disk_repo.commit_to_disk(output).map_err(DBError::from)?;
            on_disk_repo.set_is_commiting();
            // Deallocate strings and shapes that are already on disk, to save RAM
            on_disk_repo.deallocate_strings_shapes();
            output.clear();
        }

        Ok(())
    }

    /// Find a commit at the current block level minus PRESERVE_BLOCK_LEVEL.
    ///
    /// This makes sure that we don't create a snapshot from a temporary branch.
    fn find_frozen_commit(&self, commit_hash_id: HashId, bytes: &mut Vec<u8>) -> Option<HashId> {
        let mut hash_id = commit_hash_id;

        for _ in 0..PRESERVE_BLOCK_LEVEL {
            bytes.clear();
            self.with_object(hash_id, |object_bytes| {
                bytes.extend_from_slice(object_bytes);
            })
            .ok()?;

            hash_id = commit_parent_hash(bytes)?;
        }

        // Make sure that the hash_id (and its object) exists
        self.with_object(hash_id, |_| {}).ok()?;

        Some(hash_id)
    }

    #[allow(clippy::too_many_arguments)]
    fn traverse_to_make_snapshot(
        &self,
        hash_id: HashId,
        stack_hash_id: &mut ChunkedVec<HashId, 8192>,
        stack_new_refs: &mut ChunkedVec<ObjectReference, 8192>,
        storage: &mut Storage,
        strings: &mut StringInterner,
        bytes: &mut Vec<u8>,
        on_disk_repo: &mut Persistent,
        output: &mut SerializeOutput,
        stats: &mut SerializeStats,
        batch: &mut ChunkedVec<(HashId, InlinedBoxedSlice), { BATCH_CHUNK_CAPACITY }>,
        string: &mut String,
        hash: &mut ObjectHash,
    ) -> Result<ObjectReference, GCError> {
        self.maybe_write_to_disk(output, on_disk_repo)?;

        let start = stack_hash_id.len();

        // Collect all children of `hash_id` into `stack_hash_id`.
        // Every child in `stack_hash_id` has a corresponding new reference
        // in `stack_new_refs`:
        // `stack_new_refs[X]` is the new reference of `stack_hash_id[X]` in
        // the persistent context / snapshot.
        self.for_each_child(hash_id, bytes, &mut |hash_id| {
            stack_hash_id.push(hash_id);
            stack_new_refs.push(ObjectReference::default());
        })?;

        let end = stack_hash_id.len();

        for index in start..end {
            let hash_id = stack_hash_id[index];
            let new_obj_ref = self.traverse_to_make_snapshot(
                hash_id,
                stack_hash_id,
                stack_new_refs,
                storage,
                strings,
                bytes,
                on_disk_repo,
                output,
                stats,
                batch,
                string,
                hash,
            )?;

            // Set the new references of this object into `stack_new_refs`
            stack_new_refs[index] = new_obj_ref;
        }

        // Copy the object (of `hash_id`) into `bytes`.
        bytes.clear();
        self.with_object(hash_id, |object_bytes| {
            bytes.extend_from_slice(object_bytes);
        })?;

        // Deserialize the object
        let mut object = {
            let in_mem_repo = self.in_mem_repository()?.read();
            in_memory::deserialize_object(bytes, storage, strings, &*in_mem_repo)?
        };

        // When the object is an inode, `in_memory::deserialize_object` only partialy
        // stores it in the `Storage`, we need to load it fully
        match object {
            Object::Directory(dir_id) if dir_id.is_inode() => {
                let in_mem_repo = self.in_mem_repository()?.read();
                storage.dir_full_load(dir_id, strings, &*in_mem_repo)?;
            }
            _ => {}
        }

        // Set every dir_entry/inode as non commited
        storage.nodes.for_each_mut::<_, GCError>(|dir_entry| {
            dir_entry.set_commited(false);
            Ok(())
        })?;
        storage
            .thin_pointers
            .for_each_mut::<_, GCError>(|thin_pointer| {
                thin_pointer.set_commited(false);
                Ok(())
            })?;
        storage
            .fat_pointers
            .for_each_mut::<_, GCError>(|fat_pointer| {
                fat_pointer.set_commited(false);
                Ok(())
            })?;

        // Get the strings of the object, copy them into the persistent context
        // This ensure that we only write in the snapshot the useds strings
        storage
            .directories
            .for_each_mut::<_, GCError>(|(string_id, _)| {
                string.clear();
                {
                    // Read the string from the in-mem repo, lock it as short as possible
                    let in_mem_repo = self.in_mem_repository()?.read();
                    let s = in_mem_repo
                        .get_str(*string_id)
                        .ok_or(GCError::StringNotFound)?;
                    string.push_str(s.as_ref());
                }

                // Replace the old string id (from the in-mem context) to the new string id (on disk)
                let new_string_id = on_disk_repo.string_interner.make_string_id(string);
                *string_id = new_string_id;

                Ok(())
            })?;

        // Replace old references (in-mem repo) with new ones (persistent repo)
        match &mut object {
            Object::Directory(dir_id) => {
                let mut index = start;

                storage.dir_iterate_unsorted(*dir_id, |(_, dir_entry_id)| {
                    let dir_entry = storage.get_dir_entry(*dir_entry_id)?;

                    if dir_entry.is_inlined_blob() {
                        // Inlined blobs do not have a HashId or an offset
                        return Ok(());
                    }

                    let new_obj_ref = stack_new_refs[index];

                    dir_entry.set_offset(new_obj_ref.offset());
                    dir_entry.set_hash_id(new_obj_ref.hash_id());

                    assert!(dir_entry.hash_id().is_some());

                    index += 1;
                    assert!(index <= end);

                    Ok(())
                })?;
            }
            Object::Commit(commit) => {
                let new_parent_hash_id = match commit.parent_hash_id() {
                    Some(hash_id) => {
                        Some(self.copy_hash_to_snapshot(hash_id, on_disk_repo, hash)?)
                    }
                    None => None,
                };

                let root_hash_id = commit.root_ref.hash_id();
                let offset = stack_new_refs[start].offset();

                let new_root_hash_id =
                    self.copy_hash_to_snapshot(root_hash_id, on_disk_repo, hash)?;

                commit.root_ref.set_offset(offset);
                commit.root_ref.set_hash_id(new_root_hash_id);

                if let Some(new_parent_hash_id) = new_parent_hash_id {
                    commit.set_parent_hash_id(new_parent_hash_id);
                };
            }
            Object::Blob(..) => {}
        }

        let new_hash_id = self.copy_hash_to_snapshot(hash_id, on_disk_repo, hash)?;

        let offset = persistent::serialize_object(
            &object,
            new_hash_id,
            output,
            storage,
            strings,
            stats,
            batch,
            on_disk_repo,
        )?;

        let new_obj_ref = ObjectReference::new(Some(new_hash_id), offset);

        storage.clear();

        assert_eq!(end, stack_hash_id.len());
        stack_hash_id.remove_last_nelems(end - start);

        assert_eq!(end, stack_new_refs.len());
        stack_new_refs.remove_last_nelems(end - start);

        Ok(new_obj_ref)
    }

    fn take_unused(&mut self) -> Result<ChunkedVec<HashId, { UNUSED_CHUNK_CAPACITY }>, GCError> {
        // Mark all objects/hashes at `current_generation - (PRESERVE_BLOCK_LEVEL + 1)`
        // as unused/reusable

        let unused_at = self
            .current_generation
            .wrapping_sub(PRESERVE_BLOCK_LEVEL + 1);

        let mut unused = ChunkedVec::default();

        // Loop on all objects ever created, and push the unused ones in `unused`
        for (hash_id, generation) in self.objects_generation.iter_with_keys() {
            if !matches!(generation, generation if *generation == unused_at) {
                continue;
            }

            unused.push(hash_id);
        }

        // Deallocate all unused hashes/objects
        for hash_id in unused.iter().copied() {
            // `clear()` will deallocate the single hash/object.
            // If the hash/object was the last alive in the chunk, the chunk
            // will be deallocated

            let (_, _is_obj_dealloc) = self.objects_view.clear(hash_id)?;
            let (_, _is_hash_dealloc) = self.hashes_view.clear(hash_id)?;

            // Remove the generation of the HashId
            self.objects_generation
                .insert_at(hash_id, Generation::none())?;

            // Update `Self::nalives_per_chunks`
            let chunk_index = self.hashes_view.chunk_index_of(hash_id)? as u32;
            let nalive = self.nalives_per_chunk.entry(chunk_index)?;
            *nalive = nalive.checked_sub(1).ok_or(GCError::InvalidAliveState)?;
        }

        Ok(unused)
    }

    fn mark_new_ids(
        &mut self,
        new_ids: ChunkedVec<HashId, NEW_IDS_CHUNK_CAPACITY>,
    ) -> Result<(), GCError> {
        self.new_ids_in_commit += new_ids.len();

        for hash_id in new_ids.iter().copied() {
            // A `HashId` is used, update its generation
            self.objects_generation
                .insert_at(hash_id, self.current_generation)?;

            // Update `Self::nalives_per_chunks`
            let chunk_index = self.hashes_view.chunk_index_of(hash_id)? as u32;
            let nalive = self.nalives_per_chunk.entry(chunk_index)?;
            *nalive += 1;
        }

        Ok(())
    }

    fn debug_commit(&self) {
        let stats = CommitStatistics {
            new_hash_id: self.new_ids_in_commit,
        };
        log!("{:?}", stats);
    }

    fn make_snapshot_paths(&self) -> Result<(String, String), GCError> {
        let path_context = self
            .configuration
            .db_path
            .as_deref()
            .unwrap_or("/tmp/tezedge-context");

        let mut path_snapshot = PathBuf::from(path_context);
        path_snapshot.pop();
        path_snapshot.push("snapshot");

        let path_snapshot = path_snapshot
            .to_str()
            .ok_or(GCError::InvalidSnapshotPath)?
            .to_string();

        Ok((path_snapshot, path_context.to_string()))
    }

    fn make_snapshot(&mut self, commit_hash_id: HashId) -> Result<(), GCError> {
        if self.last_snapshot_timestamp.elapsed() < self.delay_between_snapshot {
            return Ok(());
        }
        self.last_snapshot_timestamp = std::time::Instant::now();

        let (path_snapshot, path_context) = self.make_snapshot_paths()?;

        log!(
            "Making snapshot snapshot_path={:?} context_path={:?}",
            path_snapshot,
            path_context
        );

        let mut hash_ids = ChunkedVec::default();
        let mut new_hash_ids = ChunkedVec::default();
        let mut storage = Storage::default();
        let mut strings = StringInterner::default();
        let mut bytes = Vec::with_capacity(16 * 1024 * 1024);
        let mut stats = SerializeStats::default();
        let mut batch = Default::default();
        let mut string = String::with_capacity(1024);
        let mut hash = ObjectHash::default();

        let commit_hash_id = self
            .find_frozen_commit(commit_hash_id, &mut bytes)
            .ok_or(GCError::FrozenCommitNotFound)?;

        let mut repository = Persistent::try_new(PersistentConfiguration {
            db_path: Some(path_snapshot.clone()),
            startup_check: false,
            read_mode: false,
        })?;

        repository.set_is_commiting();

        let file_offset = repository.data_file_offset();
        let mut output = SerializeOutput::new(Some(file_offset));

        let now = std::time::Instant::now();

        let commit_ref = self.traverse_to_make_snapshot(
            commit_hash_id,
            &mut hash_ids,
            &mut new_hash_ids,
            &mut storage,
            &mut strings,
            &mut bytes,
            &mut repository,
            &mut output,
            &mut stats,
            &mut batch,
            &mut string,
            &mut hash,
        )?;

        repository.put_context_hash(commit_ref)?;
        repository.commit_to_disk(&output).map_err(DBError::from)?;

        log!("Snapshot done in {:?}", now.elapsed());

        replace_context_with_snapshot(&path_context, &path_snapshot)?;

        Ok(())
    }

    fn handle_commit(
        &mut self,
        new_ids: ChunkedVec<HashId, NEW_IDS_CHUNK_CAPACITY>,
        block_level: u32,
        commit_hash_id: HashId,
    ) -> Result<(), GCError> {
        let now = std::time::Instant::now();

        self.napplieds_since_last_run += 1;
        self.mark_new_ids(new_ids)?;

        // Increment the current generation when the block level has been
        // incremented
        let increment_generation = block_level > self.highest_block_level;

        self.highest_block_level = self.highest_block_level.max(block_level);

        if self.debug {
            self.debug_commit();
        }

        // Reset `Self::new_ids_in_commit`
        self.new_ids_in_commit = 0;

        if !self.recv.is_empty() || self.napplieds_since_last_run < NAPPLIED_BEFORE_COLLECT {
            // Do not run the the garbage collection now.
            //
            // We enter this branch in 2 cases:
            // - There are still `Command` to process in the channel
            //   This occurs when the gc is late: the main thread applied blocks while
            //   the garbage collection was running
            // - We have to wait to process `NAPPLIED_BEFORE_COLLECT` before running
            //   the garbage collection

            self.send_unused_on_empty_channel();
            return Ok(());
        }

        self.make_snapshot(commit_hash_id)?;

        self.napplieds_since_last_run = 0;
        self.send_unused()?;

        let mut nobjects = 0;
        let mut max_depth = 0;
        let mut object_total_bytes = 0;
        let mut hash_ids = ChunkedVec::default();
        self.traverse_and_mark_tree(
            commit_hash_id,
            &mut hash_ids,
            &mut nobjects,
            1,
            &mut max_depth,
            &mut object_total_bytes,
        )?;

        let unused = self.take_unused()?;
        let unused_found = unused.len();

        self.pending.append_chunks(unused);

        if increment_generation {
            self.current_generation = self.current_generation.wrapping_increment();
        };

        if self.debug {
            let gc_duration = now.elapsed();
            let delay_since_last_gc = self.last_gc_timestamp.map(|t| t.elapsed());
            self.last_gc_timestamp.replace(std::time::Instant::now());

            let (objects_chunks_alive, objects_chunks_dead) =
                self.objects_view.count_alives_and_deads();
            let (hashes_chunks_alive, hashes_chunks_dead) =
                self.hashes_view.count_alives_and_deads();

            let stats = CollectorStatistics {
                unused_found,
                nobjects,
                max_depth,
                object_total_bytes,
                gc_duration,
                objects_chunks_alive,
                objects_chunks_dead,
                hashes_chunks_alive,
                hashes_chunks_dead,
                delay_since_last_gc,
            };

            log!("{:?}", stats);

            debug_jemalloc();
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::chunks::SharedIndexMap;

    use super::*;

    #[test]
    #[should_panic]
    fn wrapping_sub_generation_panic() {
        let zero = Generation::zero();
        zero.wrapping_sub(128);
    }

    #[test]
    #[should_panic]
    fn generation_from_panic() {
        let _ = Generation::from(128);
    }

    #[test]
    fn wrapping_inc_generation() {
        let mut n = Generation::from(127);
        for i in 1..1_000u16 {
            n = n.wrapping_increment();
            assert_eq!(n, Generation::from(((i - 1) & 0b01111111) as u8));
            assert!(!n.is_none());
        }
    }

    #[test]
    fn none_generation() {
        let none = Generation::none();
        assert!(none.is_none());
    }

    #[test]
    fn wrapping_sub_generation() {
        let zero = Generation::zero();

        for i in 1..20 {
            assert_eq!(zero.wrapping_sub(i), Generation::from(128 - i));
        }
        assert_eq!(zero.wrapping_sub(1), Generation::from(127));
        assert_eq!(zero.wrapping_sub(2), Generation::from(126));
        assert_eq!(zero.wrapping_sub(3), Generation::from(125));

        let one = zero.wrapping_increment();
        assert_eq!(one, Generation::from(1));

        for i in 2..20 {
            assert_eq!(one.wrapping_sub(i), Generation::from(128 - i + 1));
        }
        assert_eq!(one.wrapping_sub(1), Generation::from(0));
        assert_eq!(one.wrapping_sub(2), Generation::from(127));
        assert_eq!(one.wrapping_sub(3), Generation::from(126));

        let two = one.wrapping_increment();
        assert_eq!(two, Generation::from(2));

        for i in 3..20 {
            assert_eq!(two.wrapping_sub(i), Generation::from(128 - i + 2));
        }
        assert_eq!(two.wrapping_sub(1), Generation::from(1));
        assert_eq!(two.wrapping_sub(2), Generation::from(0));
        assert_eq!(two.wrapping_sub(3), Generation::from(127));
        assert_eq!(two.wrapping_sub(4), Generation::from(126));

        let n = Generation::from(127);

        for i in 0..20 {
            assert_eq!(n.wrapping_sub(i), Generation::from(127 - i));
        }
    }

    #[test]
    fn level_generation() {
        // Make sure that `GCThread::current_generation` is incremented on an increased block level
        let mut objects = SharedIndexMap::default();
        let hashes = SharedIndexMap::default();
        let id = HashId::new(10).unwrap();
        let chunks = ChunkedVec::new();
        let conf = InMemoryConfiguration {
            db_path: None,
            startup_check: false,
        };

        objects
            .insert_at(id, InlinedBoxedSlice::from(&[][..]))
            .unwrap();

        let (_sender, recv) = crossbeam_channel::unbounded();
        let (producer, _consumer) = tezos_spsc::bounded(2_000_000);
        let objects_view = objects.get_view();
        let hashes_view = hashes.get_view();

        let mut gc = GCThread::new(recv, producer, objects_view, hashes_view, conf);
        gc.debug = true;
        gc.napplieds_since_last_run = NAPPLIED_BEFORE_COLLECT + 1;

        gc.add_chunks(objects.clone_new_chunks(), None);

        let before = gc.current_generation.inner;

        {
            const BLOCK_LEVEL: u32 = 10;
            gc.handle_commit(chunks.clone(), BLOCK_LEVEL, id).unwrap();
            assert_eq!(gc.current_generation, Generation::from(before + 1));

            gc.napplieds_since_last_run = NAPPLIED_BEFORE_COLLECT + 1;

            // Different commits at the same `block_level`
            gc.handle_commit(chunks.clone(), BLOCK_LEVEL, id).unwrap();
            gc.napplieds_since_last_run = NAPPLIED_BEFORE_COLLECT + 1;
            gc.handle_commit(chunks.clone(), BLOCK_LEVEL, id).unwrap();
            assert_eq!(gc.current_generation, Generation::from(before + 1));
        }

        {
            gc.napplieds_since_last_run = NAPPLIED_BEFORE_COLLECT + 1;

            // New `block_level`
            const BLOCK_LEVEL: u32 = 11;
            gc.handle_commit(chunks.clone(), BLOCK_LEVEL, id).unwrap();
            assert_eq!(gc.current_generation, Generation::from(before + 2));
            // Same `block_level`
            gc.napplieds_since_last_run = NAPPLIED_BEFORE_COLLECT + 1;
            gc.handle_commit(chunks.clone(), BLOCK_LEVEL, id).unwrap();
            gc.napplieds_since_last_run = NAPPLIED_BEFORE_COLLECT + 1;
            gc.handle_commit(chunks.clone(), BLOCK_LEVEL, id).unwrap();
            assert_eq!(gc.current_generation, Generation::from(before + 2));
        }

        {
            // Previous `block_level`
            const BLOCK_LEVEL: u32 = 10;
            gc.napplieds_since_last_run = NAPPLIED_BEFORE_COLLECT + 1;
            gc.handle_commit(chunks.clone(), BLOCK_LEVEL, id).unwrap();
            assert_eq!(gc.current_generation, Generation::from(before + 2));
            // Same `block_level`
            gc.napplieds_since_last_run = NAPPLIED_BEFORE_COLLECT + 1;
            gc.handle_commit(chunks.clone(), BLOCK_LEVEL, id).unwrap();
            gc.napplieds_since_last_run = NAPPLIED_BEFORE_COLLECT + 1;
            gc.handle_commit(chunks.clone(), BLOCK_LEVEL, id).unwrap();
            assert_eq!(gc.current_generation, Generation::from(before + 2));
        }

        {
            // Previous `block_level`
            const BLOCK_LEVEL: u32 = 11;
            gc.napplieds_since_last_run = NAPPLIED_BEFORE_COLLECT + 1;
            gc.handle_commit(chunks.clone(), BLOCK_LEVEL, id).unwrap();
            assert_eq!(gc.current_generation, Generation::from(before + 2));
            // Same `block_level`
            gc.napplieds_since_last_run = NAPPLIED_BEFORE_COLLECT + 1;
            gc.handle_commit(chunks.clone(), BLOCK_LEVEL, id).unwrap();
            gc.napplieds_since_last_run = NAPPLIED_BEFORE_COLLECT + 1;
            gc.handle_commit(chunks.clone(), BLOCK_LEVEL, id).unwrap();
            assert_eq!(gc.current_generation, Generation::from(before + 2));
        }

        {
            // New `block_level`
            const BLOCK_LEVEL: u32 = 12;
            gc.napplieds_since_last_run = NAPPLIED_BEFORE_COLLECT + 1;
            gc.handle_commit(chunks.clone(), BLOCK_LEVEL, id).unwrap();
            assert_eq!(gc.current_generation, Generation::from(before + 3));
            // Same `block_level`
            gc.napplieds_since_last_run = NAPPLIED_BEFORE_COLLECT + 1;
            gc.handle_commit(chunks.clone(), BLOCK_LEVEL, id).unwrap();
            gc.napplieds_since_last_run = NAPPLIED_BEFORE_COLLECT + 1;
            gc.handle_commit(chunks, BLOCK_LEVEL, id).unwrap();
            assert_eq!(gc.current_generation, Generation::from(before + 3));
        }
    }
}
