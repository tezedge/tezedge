// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! A `IndexMap` that can be shared with different threads.
//!
//! The `SharedIndexMap` itself is not shareable, but the chunks it contains are.
//! This is to avoid having to lock/synchronize the list of chunks.
//!
//! `SharedIndexMapView` allows to read the map (`SharedIndexMap`) from another thread.
//! Chunks must be send from the `SharedIndexMap` to the `SharedIndexMapView` with
//! the 2 methods `SharedIndexMap::clone_new_chunks()` and
//! `SharedIndexMapView::append_chunks()`
//!
//! Example:
//! ```
//! use tezos_context::chunks::SharedIndexMap;
//!
//! let (sender, recv) = std::sync::mpsc::sync_channel(10);
//!
//! let mut shared_index_map = SharedIndexMap::<usize, Option<usize>, 1_000>::default();
//! let mut map_view = shared_index_map.get_view();
//!
//! std::thread::spawn(move || {
//!     let chunks = recv.recv().unwrap();
//!     map_view.append_chunks(chunks);
//!
//!     let value_101 = map_view.with(101, |value| value.cloned()).unwrap();
//!     assert_eq!(value_101.unwrap().unwrap(), 121)
//! });
//!
//! shared_index_map.insert_at(101, 121).unwrap();
//!
//! let value_101 = shared_index_map.with(101, |value| value.cloned()).unwrap();
//! assert_eq!(value_101.unwrap().unwrap(), 121);
//!
//! let chunks = shared_index_map.clone_new_chunks().unwrap();
//! sender.send(chunks).unwrap();
//! ```
//!

use std::{
    convert::{TryFrom, TryInto},
    marker::PhantomData,
    sync::Arc,
};

use parking_lot::RwLock;
use static_assertions::assert_eq_size;
use thiserror::Error;

use super::DEFAULT_LIST_LENGTH;

#[derive(Debug)]
struct VecAliveCounter<T, const CHUNK_CAPACITY: usize> {
    /// Number of items alive in the container
    alive_counter: u32,
    /// Number of items in `Self::inner`
    ///
    /// Used to keep track where the `Self::push` should insert
    /// the item
    length: u32,
    /// Array of items
    inner: Option<Box<[T; CHUNK_CAPACITY]>>,
}

assert_eq_size!([u8; 16], VecAliveCounter<u8, 10>);

impl<T, const CHUNK_CAPACITY: usize> std::ops::Deref for VecAliveCounter<T, CHUNK_CAPACITY> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        match self.inner.as_ref() {
            Some(inner) => &inner[..self.length as usize],
            None => &[],
        }
    }
}

impl<T, const CHUNK_CAPACITY: usize> std::ops::DerefMut for VecAliveCounter<T, CHUNK_CAPACITY> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self.inner.as_mut() {
            Some(inner) => &mut inner[..self.length as usize],
            None => &mut [],
        }
    }
}

impl<T, const CHUNK_CAPACITY: usize> VecAliveCounter<Option<T>, CHUNK_CAPACITY> {
    const INIT: Option<T> = None;

    fn push(&mut self, elem: Option<T>) {
        let inner = match self.inner.as_mut() {
            Some(inner) => inner,
            None => {
                self.allocate(false);
                self.inner.as_mut().unwrap() // Never fails, we just allocated it
            }
        };

        assert!((self.length as usize) < CHUNK_CAPACITY);

        inner[self.length as usize] = elem;
        self.length += 1;
    }

    fn allocate(&mut self, fill: bool) {
        assert!(self.is_deallocated());
        self.inner = Some(Box::from([Self::INIT; CHUNK_CAPACITY]));
        if fill {
            self.length = CHUNK_CAPACITY as u32;
        }
    }
}

impl<T, const CHUNK_CAPACITY: usize> VecAliveCounter<T, CHUNK_CAPACITY> {
    fn deallocate(&mut self) {
        self.inner = None;
        self.length = 0;
    }

    fn is_deallocated(&self) -> bool {
        self.inner.is_none()
    }
}

impl<T, const CHUNK_CAPACITY: usize> Default for VecAliveCounter<T, CHUNK_CAPACITY> {
    fn default() -> Self {
        Self {
            alive_counter: 0,
            length: 0,
            inner: None,
        }
    }
}

/// A `Send` + `Sync` chunk
#[derive(Debug)]
pub struct SharedChunk<T, const CHUNK_CAPACITY: usize> {
    inner: Arc<RwLock<VecAliveCounter<T, CHUNK_CAPACITY>>>,
}

assert_eq_size!([u8; 24], RwLock<VecAliveCounter<u8, 10>>);

impl<T, const CHUNK_CAPACITY: usize> Clone for SharedChunk<T, CHUNK_CAPACITY> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<T, const CHUNK_CAPACITY: usize> Default for SharedChunk<T, CHUNK_CAPACITY> {
    fn default() -> Self {
        Self {
            inner: Default::default(),
        }
    }
}

impl<T, const CHUNK_CAPACITY: usize> SharedChunk<T, CHUNK_CAPACITY> {
    fn with<F, R>(&self, index: usize, fun: F) -> R
    where
        F: FnOnce(Option<&T>) -> R,
    {
        let inner = self.inner.read();
        fun(Some(&inner[index]))
    }

    fn len(&self) -> usize {
        self.inner.read().len()
    }
}

impl<T, const CHUNK_CAPACITY: usize> SharedChunk<Option<T>, CHUNK_CAPACITY> {
    fn push(&self, elem: Option<T>) -> usize {
        let mut inner = self.inner.write();

        let index = inner.len();

        if elem.is_some() {
            inner.alive_counter += 1;
        }
        inner.push(elem);

        index
    }

    /// Remove an item from the chunk
    ///
    /// If the item is the last alive (`Option::Some(_)`), the chunk is deallocated
    fn clear(
        &self,
        index: usize,
        is_last_chunk: bool,
    ) -> Result<(Option<T>, bool), SharedIndexMapError> {
        let mut inner = self.inner.write();

        let old = match inner.get_mut(index).and_then(std::mem::take) {
            Some(old) => old,
            None => return Ok((None, false)),
        };

        inner.alive_counter = match inner.alive_counter.checked_sub(1) {
            Some(counter) => counter,
            None => return Err(SharedIndexMapError::InvalidAliveState),
        };

        // If the chunk is empty, deallocate it
        // Do not deallocate if we are the last chunk, this would render
        // `SharedChunkedVec::nelems` invalid
        if inner.alive_counter == 0 && !is_last_chunk {
            inner.deallocate();
            return Ok((Some(old), true));
        }

        Ok((Some(old), false))
    }

    fn insert_alive_at(&self, index: usize, value: Option<T>) {
        let mut inner = self.inner.write();

        if inner.is_deallocated() {
            assert_eq!(inner.alive_counter, 0);
            inner.allocate(true);
        }

        if std::mem::replace(&mut inner[index], value).is_none() {
            inner.alive_counter += 1;
        }

        assert!(inner.alive_counter as usize <= CHUNK_CAPACITY);
    }
}

#[derive(Debug)]
pub struct SharedChunkedVec<T, const CHUNK_CAPACITY: usize> {
    pub list_of_chunks: Vec<SharedChunk<T, CHUNK_CAPACITY>>,
    /// Number of elements in the chunks
    nelems: usize,
    /// Index in `Self::list_of_chunks` that was synchronized
    /// with `Self::clone_new_chunks`
    synced_at: usize,
}

impl<T, const CHUNK_CAPACITY: usize> Default for SharedChunkedVec<T, CHUNK_CAPACITY> {
    fn default() -> Self {
        assert_ne!(CHUNK_CAPACITY, 0);

        let list_of_chunks =
            Vec::<SharedChunk<T, CHUNK_CAPACITY>>::with_capacity(DEFAULT_LIST_LENGTH);

        Self {
            list_of_chunks,
            nelems: 0,
            synced_at: 0,
        }
    }
}

impl<T, const CHUNK_CAPACITY: usize> SharedChunkedVec<T, CHUNK_CAPACITY> {
    pub fn empty() -> Self {
        Self {
            list_of_chunks: Vec::new(),
            nelems: 0,
            synced_at: 0,
        }
    }

    fn append_chunks(&mut self, mut chunks: Vec<SharedChunk<T, CHUNK_CAPACITY>>) {
        self.list_of_chunks.append(&mut chunks)
    }

    pub fn clone_new_chunks(&mut self) -> Option<Vec<SharedChunk<T, CHUNK_CAPACITY>>> {
        let new_chunks = self.list_of_chunks.get(self.synced_at..)?;

        if new_chunks.is_empty() {
            return None;
        }

        self.synced_at = self.list_of_chunks.len();

        Some(new_chunks.to_vec())
    }

    /// Returns the last chunk with space available.
    ///
    /// Allocates one more chunk in 2 cases:
    /// - The last chunk has reached `Self::chunk_capacity` limit
    /// - `Self::list_of_chunks` is empty
    fn get_next_chunk(&mut self) -> &mut SharedChunk<T, CHUNK_CAPACITY> {
        let must_alloc_new_chunk = self
            .list_of_chunks
            .last()
            .map(|chunk| {
                debug_assert!(chunk.len() <= CHUNK_CAPACITY);
                chunk.len() == CHUNK_CAPACITY
            })
            .unwrap_or(true);

        if must_alloc_new_chunk {
            self.list_of_chunks.push(SharedChunk::default());
        }

        // Never fail, we just allocated one in case it's empty
        self.list_of_chunks.last_mut().unwrap()
    }

    pub fn capacity(&self) -> usize {
        CHUNK_CAPACITY * self.list_of_chunks.len()
    }

    fn get_indexes_at(&self, index: usize) -> (usize, usize) {
        let list_index = index / CHUNK_CAPACITY;
        let chunk_index = index % CHUNK_CAPACITY;

        (list_index, chunk_index)
    }

    pub fn len(&self) -> usize {
        self.nelems
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn with<F, R>(&self, index: usize, fun: F) -> R
    where
        F: FnOnce(Option<&T>) -> R,
    {
        let (list_index, chunk_index) = self.get_indexes_at(index);

        self.list_of_chunks[list_index].with(chunk_index, fun)
    }
}

impl<T, const CHUNK_CAPACITY: usize> SharedChunkedVec<Option<T>, CHUNK_CAPACITY> {
    fn clear(&self, index: usize) -> Result<(Option<T>, bool), SharedIndexMapError> {
        let (list_index, chunk_index) = self.get_indexes_at(index);

        let chunk = match self.list_of_chunks.get(list_index) {
            Some(chunk) => chunk,
            None => return Ok((None, false)),
        };
        let is_last_chunk = list_index + 1 == self.list_of_chunks.len();

        chunk.clear(chunk_index, is_last_chunk)
    }

    pub fn push(&mut self, elem: Option<T>) -> usize {
        let index = self.len();
        self.nelems += 1;

        let index_in_chunk = self.get_next_chunk().push(elem);
        let list_index = self.list_of_chunks.len() - 1;

        assert_eq!((list_index * CHUNK_CAPACITY) + index_in_chunk, index);

        index
    }

    pub fn resize_with(&mut self, new_len: usize) {
        while self.nelems < new_len {
            self.push(None);
        }
    }

    fn insert_at(&self, index: usize, value: Option<T>) {
        let (list_index, chunk_index) = self.get_indexes_at(index);
        self.list_of_chunks[list_index].insert_alive_at(chunk_index, value);
    }
}

#[derive(Debug, Error)]
pub enum SharedIndexMapError {
    #[error("Invalid state: the alive count is invalid")]
    InvalidAliveState,
    #[error("Fail to convert index")]
    Indexing,
}

#[derive(Debug)]
pub struct SharedIndexMap<K, V, const CHUNK_CAPACITY: usize> {
    pub entries: SharedChunkedVec<V, CHUNK_CAPACITY>,
    _phantom: PhantomData<K>,
}

impl<K, V, const CHUNK_CAPACITY: usize> Default for SharedIndexMap<K, V, CHUNK_CAPACITY> {
    fn default() -> Self {
        Self {
            entries: SharedChunkedVec::default(),
            _phantom: PhantomData,
        }
    }
}

impl<K, V, const CHUNK_CAPACITY: usize> SharedIndexMap<K, V, CHUNK_CAPACITY> {
    pub fn empty() -> Self {
        Self {
            entries: SharedChunkedVec::empty(),
            _phantom: PhantomData,
        }
    }

    pub fn get_view(&self) -> SharedIndexMapView<K, V, CHUNK_CAPACITY> {
        SharedIndexMapView {
            inner: Self::default(),
        }
    }

    fn append_chunks(&mut self, chunks: Vec<SharedChunk<V, CHUNK_CAPACITY>>) {
        self.entries.append_chunks(chunks)
    }

    pub fn clone_new_chunks(&mut self) -> Option<Vec<SharedChunk<V, CHUNK_CAPACITY>>> {
        self.entries.clone_new_chunks()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn capacity(&self) -> usize {
        self.entries.capacity()
    }

    /// Used for logging
    pub fn count_alives_and_deads(&self) -> (usize, usize) {
        let mut alive = 0;
        let mut dead = 0;

        for chunk in &self.entries.list_of_chunks {
            if chunk.inner.read().is_empty() {
                dead += 1;
            } else {
                alive += 1;
            }
        }

        (alive, dead)
    }
}

impl<K, V, const CHUNK_CAPACITY: usize> SharedIndexMap<K, V, CHUNK_CAPACITY>
where
    K: TryInto<usize>,
{
    pub fn contains_key(&self, key: K) -> Result<bool, K::Error> {
        let index = key.try_into()?;
        Ok(index < self.entries.len())
    }

    pub fn with<F, R>(&self, key: K, fun: F) -> Result<R, K::Error>
    where
        F: FnOnce(Option<&V>) -> R,
    {
        let index = key.try_into()?;
        Ok(self.entries.with(index, fun))
    }

    pub fn chunk_index_of(&self, key: K) -> Result<usize, K::Error> {
        let index: usize = key.try_into()?;
        Ok(index / CHUNK_CAPACITY)
    }
}

impl<K, V, const CHUNK_CAPACITY: usize> SharedIndexMap<K, Option<V>, CHUNK_CAPACITY>
where
    K: TryFrom<usize>,
    K: TryInto<usize>,
    K: Clone,
{
    pub fn push(&mut self, value: V) -> Result<K, <K as TryFrom<usize>>::Error> {
        let index = self.entries.push(Some(value));
        let key = K::try_from(index);

        debug_assert!(self
            .with(key.as_ref().ok().unwrap().clone(), |v| v.is_some())
            .ok()
            .unwrap());

        key
    }
}

impl<K, V, const CHUNK_CAPACITY: usize> SharedIndexMap<K, Option<V>, CHUNK_CAPACITY>
where
    K: TryInto<usize> + Clone,
{
    pub fn clear(&self, key: K) -> Result<(Option<V>, bool), SharedIndexMapError> {
        let index = key.try_into().map_err(|_| SharedIndexMapError::Indexing)?;
        self.entries.clear(index)
    }

    pub fn insert_at(&mut self, key: K, value: V) -> Result<(), K::Error> {
        let index: usize = key.try_into()?;

        if index >= self.entries.len() {
            self.entries.resize_with(index + 1);
        }

        self.entries.insert_at(index, Some(value));

        Ok(())
    }
}

/// It's a `SharedIndexMap` with a limited API
///
/// Compared to `SharedIndexMap`, it cannot modify the number of elements
/// in the container
///
/// `SharedIndexMap` and `SharedIndexMapView` must be synchronized with
/// `SharedIndexMap::clone_new_chunks` and `SharedIndexMapView::append_chunks`
pub struct SharedIndexMapView<K, V, const CHUNK_CAPACITY: usize> {
    inner: SharedIndexMap<K, V, CHUNK_CAPACITY>,
}

impl<K, V, const CHUNK_CAPACITY: usize> SharedIndexMapView<K, V, CHUNK_CAPACITY> {
    pub fn append_chunks(&mut self, chunks: Vec<SharedChunk<V, CHUNK_CAPACITY>>) {
        self.inner.append_chunks(chunks)
    }

    pub fn nchunks(&self) -> usize {
        self.inner.entries.list_of_chunks.len()
    }

    pub fn count_alives_and_deads(&self) -> (usize, usize) {
        self.inner.count_alives_and_deads()
    }
}

impl<K, V, const CHUNK_CAPACITY: usize> SharedIndexMapView<K, Option<V>, CHUNK_CAPACITY>
where
    K: TryInto<usize> + Clone,
{
    pub fn clear(&self, key: K) -> Result<(Option<V>, bool), SharedIndexMapError> {
        self.inner.clear(key)
    }
}

impl<K, V, const CHUNK_CAPACITY: usize> SharedIndexMapView<K, V, CHUNK_CAPACITY>
where
    K: TryInto<usize>,
{
    pub fn with<F, R>(&self, key: K, fun: F) -> Result<R, K::Error>
    where
        F: FnOnce(Option<&V>) -> R,
    {
        self.inner.with(key, fun)
    }

    pub fn chunk_index_of(&self, key: K) -> Result<usize, K::Error> {
        self.inner.chunk_index_of(key)
    }
}
