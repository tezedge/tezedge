// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::ops::{Index, Range};

use super::{Chunk, DEFAULT_LIST_LENGTH};

/// Structure allocating multiple `Chunk`, its values are accessible (only) by range
///
/// Example:
/// ```
/// use tezos_context::chunks::ChunkedSlice;
///
/// let mut chunks = ChunkedSlice::with_chunk_capacity(1000);
/// let (start, length) = chunks.extend_from_slice(&[1, 2, 3]);
/// assert_eq!(chunks.get(start..start + length).unwrap(), &[1, 2, 3])
/// ```
#[derive(Debug)]
pub struct ChunkedSlice<T> {
    list_of_chunks: Vec<Chunk<T>>,
    chunk_capacity: usize,
    /// Number of elements in the chunks
    nelems: usize,
}

impl<T> Index<Range<usize>> for ChunkedSlice<T> {
    type Output = [T];

    fn index(&self, Range { start, end }: Range<usize>) -> &Self::Output {
        let length = end - start;
        let (list_index, chunk_index) = self.get_indexes_at(start);

        &self.list_of_chunks[list_index][chunk_index..chunk_index + length]
    }
}

impl<T> ChunkedSlice<T>
where
    T: Clone,
{
    /// Extends the last chunk with `slice`
    ///
    /// Return the index of the slice in the chunks, and its length
    pub fn extend_from_slice(&mut self, slice: &[T]) -> (usize, usize) {
        self.maybe_alloc_chunk();

        let start = self.get_start();
        let slice_length = slice.len();
        self.nelems += slice_length;

        self.list_of_chunks
            .last_mut()
            .unwrap() // Never fail, we called `Self::maybe_alloc_chunk`
            .extend_from_slice(slice);

        (start, slice_length)
    }
}

impl<T> ChunkedSlice<T> {
    /// Returns a new `ChunkedSlice<T>` without allocating
    pub fn empty() -> Self {
        Self {
            list_of_chunks: Vec::new(),
            chunk_capacity: 1_000,
            nelems: 0,
        }
    }

    pub fn with_chunk_capacity(chunk_capacity: usize) -> Self {
        assert_ne!(chunk_capacity, 0);

        let chunk: Vec<T> = Vec::with_capacity(chunk_capacity);

        let mut list_of_vec: Vec<Chunk<T>> = Vec::with_capacity(DEFAULT_LIST_LENGTH);
        list_of_vec.push(chunk);

        Self {
            list_of_chunks: list_of_vec,
            chunk_capacity,
            nelems: 0,
        }
    }

    pub fn capacity(&self) -> usize {
        self.chunk_capacity * self.list_of_chunks.len()
    }

    /// Allocates one more chunk in 2 cases:
    /// - The last chunk has reached `Self::chunk_capacity` limit
    /// - `Self::list_of_chunks` is empty
    fn maybe_alloc_chunk(&mut self) {
        let chunk_capacity = self.chunk_capacity;

        if self
            .list_of_chunks
            .last_mut()
            .map(|chunk| chunk.len() >= chunk_capacity)
            .unwrap_or(true)
        {
            self.list_of_chunks
                .push(Vec::with_capacity(self.chunk_capacity));
        }
    }

    /// Appends `other` in the last chunk.
    ///
    /// Return the index of the slice in the chunks, and its length
    pub fn append(&mut self, other: &mut Vec<T>) -> (usize, usize) {
        self.maybe_alloc_chunk();

        let start = self.get_start();
        let other_length = other.len();
        self.nelems += other_length;

        // Never fail, we called `Self::maybe_alloc_chunk`
        self.list_of_chunks.last_mut().unwrap().append(other);

        (start, other_length)
    }

    pub fn get(&self, Range { start, end }: Range<usize>) -> Option<&[T]> {
        let length = end - start;
        let (list_index, chunk_index) = self.get_indexes_at(start);

        self.list_of_chunks
            .get(list_index)?
            .get(chunk_index..chunk_index + length)
    }

    fn get_indexes_at(&self, index: usize) -> (usize, usize) {
        let list_index = index / self.chunk_capacity;
        let chunk_index = index % self.chunk_capacity;

        (list_index, chunk_index)
    }

    /// Removes `nelems` from the last chunk.
    pub fn remove_last_nelems(&mut self, nelems: usize) {
        if let Some(chunk) = self.list_of_chunks.last_mut() {
            chunk.truncate(chunk.len() - nelems);
            self.nelems -= nelems;
        };
    }

    /// Returns the index of the next slice to be pushed in the chunks.
    ///
    /// This must be called after any `Self::maybe_alloc_chunk`.
    fn get_start(&self) -> usize {
        let nfull_chunk = self.list_of_chunks.len().saturating_sub(1);
        let last_chunk_length = self.list_of_chunks.last().map(Vec::len).unwrap_or(0);

        (nfull_chunk * self.chunk_capacity) + last_chunk_length
    }

    pub fn nelems(&self) -> usize {
        self.nelems
    }

    pub fn deallocate(&mut self) {
        self.list_of_chunks = Vec::new();
        self.nelems = 0;
    }

    pub fn clear(&mut self) {
        self.list_of_chunks.truncate(1);
        if let Some(first_chunk) = self.list_of_chunks.last_mut() {
            first_chunk.clear();
        };
        self.nelems = 0;
    }
}
