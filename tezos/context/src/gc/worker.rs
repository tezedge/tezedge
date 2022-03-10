// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    num::TryFromIntError,
    sync::atomic::{AtomicUsize, Ordering},
    time::Instant,
};

use crossbeam_channel::{Receiver, RecvError};
use static_assertions::assert_eq_size;
use thiserror::Error;

use crate::{
    chunks::{ChunkedVec, SharedChunk, SharedIndexMapError, SharedIndexMapView},
    gc::{
        jemalloc::debug_jemalloc,
        stats::{CollectorStatistics, CommitStatistics, OnMessageStatistics},
    },
    kv_store::{
        in_memory::OBJECTS_CHUNK_CAPACITY, index_map::IndexMap,
        inline_boxed_slice::InlinedBoxedSlice, HashId, HashIdError,
    },
    serialize::in_memory::iter_hash_ids,
    ObjectHash,
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

/// Used for statistics
///
/// Number of items in `GCThread::pending`.
pub(crate) static GC_PENDING_HASHIDS: AtomicUsize = AtomicUsize::new(0);

type ChunkIndex = u32;

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
    objects_generation: IndexMap<HashId, Option<u8>, 1_000_000>,
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
    current_generation: u8,

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
    Close,
}

impl GCThread {
    pub fn new(
        recv: Receiver<Command>,
        producer: Producer<HashId>,
        objects_view: SharedIndexMapView<HashId, Option<InlinedBoxedSlice>, OBJECTS_CHUNK_CAPACITY>,
        hashes_view: SharedIndexMapView<HashId, Option<Box<ObjectHash>>, OBJECTS_CHUNK_CAPACITY>,
    ) -> Self {
        Self {
            recv,
            free_ids: producer,
            pending: ChunkedVec::default(),
            debug: false,
            objects_generation: IndexMap::default(),
            current_generation: 0,
            objects_view,
            hashes_view,
            nalives_per_chunk: IndexMap::default(),
            new_ids_in_commit: 0,
            last_gc_timestamp: None,
            napplieds_since_last_run: 0,
            highest_block_level: 0,
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

    #[allow(clippy::too_many_arguments)]
    fn traverse_and_mark_tree(
        &mut self,
        hash_id: HashId,
        current_generation: u8,
        nobjects: &mut usize,
        depth: usize,
        max_depth: &mut usize,
        objets_total_bytes: &mut usize,
    ) -> Result<(), GCError> {
        *nobjects += 1;
        *max_depth = depth.max(*max_depth);

        // Store the `HashId` of the children in this array, we avoid
        // allocating a `Vec`
        // The maximum depth of recursion is currently `16`, so a stack
        // overflow will not occurs.
        // Note: We could create a container that allocate a `Vec` when
        // some `depth` is reached
        let mut hash_ids = [None::<HashId>; 256];

        self.objects_view.with(hash_id, |object_bytes| {
            let object_bytes = match object_bytes {
                Some(Some(object_bytes)) => object_bytes,
                _ => {
                    elog!("Missing Object object={:?}", object_bytes);
                    return Err(GCError::TraverseTreeError);
                }
            };

            *objets_total_bytes += object_bytes.len();

            // Store the `HashId` of all the children in `hash_ids`
            for (index, hash_id) in iter_hash_ids(object_bytes).enumerate() {
                hash_ids[index] = Some(hash_id);
            }

            Ok(())
        })??;

        {
            // Update the generation of the HashId (object/hash)
            self.objects_generation
                .entry(hash_id)?
                .replace(current_generation);
        }

        // Recursively update generation of all the children
        for hash_id in hash_ids {
            let hash_id = match hash_id {
                Some(hash_id) => hash_id,
                None => break,
            };
            self.traverse_and_mark_tree(
                hash_id,
                current_generation,
                nobjects,
                depth + 1,
                max_depth,
                objets_total_bytes,
            )?;
        }

        Ok(())
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
            if !matches!(generation, Some(generation) if *generation == unused_at) {
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
            self.objects_generation.insert_at(hash_id, None)?;

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
                .insert_at(hash_id, Some(self.current_generation))?;

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

        self.napplieds_since_last_run = 0;
        self.send_unused()?;

        let mut nobjects = 0;
        let mut max_depth = 0;
        let mut object_total_bytes = 0;
        self.traverse_and_mark_tree(
            commit_hash_id,
            self.current_generation,
            &mut nobjects,
            1,
            &mut max_depth,
            &mut object_total_bytes,
        )?;

        let unused = self.take_unused()?;
        let unused_found = unused.len();

        self.pending.append_chunks(unused);

        if increment_generation {
            self.current_generation = self.current_generation.wrapping_add(1);
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
    fn level_generation() {
        // Make sure that `GCThread::current_generation` is incremented on an increased block level
        let mut objects = SharedIndexMap::default();
        let hashes = SharedIndexMap::default();
        let id = HashId::new(10).unwrap();
        let chunks = ChunkedVec::new();

        objects
            .insert_at(id, InlinedBoxedSlice::from(&[][..]))
            .unwrap();

        let (_sender, recv) = crossbeam_channel::unbounded();
        let (producer, _consumer) = tezos_spsc::bounded(2_000_000);
        let objects_view = objects.get_view();
        let hashes_view = hashes.get_view();

        let mut gc = GCThread::new(recv, producer, objects_view, hashes_view);
        gc.debug = true;
        gc.napplieds_since_last_run = NAPPLIED_BEFORE_COLLECT + 1;

        gc.add_chunks(objects.clone_new_chunks(), None);

        let before = gc.current_generation;

        {
            const BLOCK_LEVEL: u32 = 10;
            gc.handle_commit(chunks.clone(), BLOCK_LEVEL, id).unwrap();
            assert_eq!(gc.current_generation, before + 1);

            gc.napplieds_since_last_run = NAPPLIED_BEFORE_COLLECT + 1;

            // Different commits at the same `block_level`
            gc.handle_commit(chunks.clone(), BLOCK_LEVEL, id).unwrap();
            gc.napplieds_since_last_run = NAPPLIED_BEFORE_COLLECT + 1;
            gc.handle_commit(chunks.clone(), BLOCK_LEVEL, id).unwrap();
            assert_eq!(gc.current_generation, before + 1);
        }

        {
            gc.napplieds_since_last_run = NAPPLIED_BEFORE_COLLECT + 1;

            // New `block_level`
            const BLOCK_LEVEL: u32 = 11;
            gc.handle_commit(chunks.clone(), BLOCK_LEVEL, id).unwrap();
            assert_eq!(gc.current_generation, before + 2);
            // Same `block_level`
            gc.napplieds_since_last_run = NAPPLIED_BEFORE_COLLECT + 1;
            gc.handle_commit(chunks.clone(), BLOCK_LEVEL, id).unwrap();
            gc.napplieds_since_last_run = NAPPLIED_BEFORE_COLLECT + 1;
            gc.handle_commit(chunks.clone(), BLOCK_LEVEL, id).unwrap();
            assert_eq!(gc.current_generation, before + 2);
        }

        {
            // Previous `block_level`
            const BLOCK_LEVEL: u32 = 10;
            gc.napplieds_since_last_run = NAPPLIED_BEFORE_COLLECT + 1;
            gc.handle_commit(chunks.clone(), BLOCK_LEVEL, id).unwrap();
            assert_eq!(gc.current_generation, before + 2);
            // Same `block_level`
            gc.napplieds_since_last_run = NAPPLIED_BEFORE_COLLECT + 1;
            gc.handle_commit(chunks.clone(), BLOCK_LEVEL, id).unwrap();
            gc.napplieds_since_last_run = NAPPLIED_BEFORE_COLLECT + 1;
            gc.handle_commit(chunks.clone(), BLOCK_LEVEL, id).unwrap();
            assert_eq!(gc.current_generation, before + 2);
        }

        {
            // Previous `block_level`
            const BLOCK_LEVEL: u32 = 11;
            gc.napplieds_since_last_run = NAPPLIED_BEFORE_COLLECT + 1;
            gc.handle_commit(chunks.clone(), BLOCK_LEVEL, id).unwrap();
            assert_eq!(gc.current_generation, before + 2);
            // Same `block_level`
            gc.napplieds_since_last_run = NAPPLIED_BEFORE_COLLECT + 1;
            gc.handle_commit(chunks.clone(), BLOCK_LEVEL, id).unwrap();
            gc.napplieds_since_last_run = NAPPLIED_BEFORE_COLLECT + 1;
            gc.handle_commit(chunks.clone(), BLOCK_LEVEL, id).unwrap();
            assert_eq!(gc.current_generation, before + 2);
        }

        {
            // New `block_level`
            const BLOCK_LEVEL: u32 = 12;
            gc.napplieds_since_last_run = NAPPLIED_BEFORE_COLLECT + 1;
            gc.handle_commit(chunks.clone(), BLOCK_LEVEL, id).unwrap();
            assert_eq!(gc.current_generation, before + 3);
            // Same `block_level`
            gc.napplieds_since_last_run = NAPPLIED_BEFORE_COLLECT + 1;
            gc.handle_commit(chunks.clone(), BLOCK_LEVEL, id).unwrap();
            gc.napplieds_since_last_run = NAPPLIED_BEFORE_COLLECT + 1;
            gc.handle_commit(chunks, BLOCK_LEVEL, id).unwrap();
            assert_eq!(gc.current_generation, before + 3);
        }
    }
}
