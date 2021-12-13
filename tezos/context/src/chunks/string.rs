// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{borrow::Cow, ops::Range};

use super::DEFAULT_LIST_LENGTH;

/// Structure similar to `ChunkedVec` but using `String` instead of `Vec<T>`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkedString {
    list_of_chunks: Vec<String>,
    chunk_capacity: usize,
    /// Number of bytes
    nbytes: usize,
}

impl ChunkedString {
    /// Returns a new `ChunkedSlice<T>` without allocating
    pub fn empty() -> Self {
        Self {
            list_of_chunks: Vec::new(),
            chunk_capacity: 1_000,
            nbytes: 0,
        }
    }

    pub fn with_chunk_capacity(chunk_capacity: usize) -> Self {
        assert_ne!(chunk_capacity, 0);

        let chunk = String::with_capacity(chunk_capacity);

        let mut list_of_vec: Vec<String> = Vec::with_capacity(DEFAULT_LIST_LENGTH);
        list_of_vec.push(chunk);

        Self {
            list_of_chunks: list_of_vec,
            chunk_capacity,
            nbytes: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.nbytes == 0
    }

    pub fn extend_from(&mut self, other: &Self) {
        let our_length = self.list_of_chunks.len();
        let other_length = other.list_of_chunks.len();

        if our_length != other_length {
            assert!(our_length < other_length);
            self.list_of_chunks
                .resize_with(other_length, Default::default);
        }

        let our_length = our_length.saturating_sub(1);
        let mut nbytes = 0;

        for (ours, other) in self.list_of_chunks[our_length..]
            .iter_mut()
            .zip(&other.list_of_chunks[our_length..])
        {
            let ours_length = ours.len();
            if ours_length < other.len() {
                nbytes += other.len() - ours_length;
                ours.push_str(&other[ours_length..]);
            }
        }

        self.nbytes += nbytes;
        assert_eq!(self.nbytes, other.nbytes);
    }

    /// Extends the last chunk with `slice`
    ///
    /// Return the index of the slice in the chunks, and its length
    pub fn push_str(&mut self, slice: &str) -> (usize, usize) {
        let start = self.len();
        let slice_length = slice.len();
        let chunk_capacity = self.chunk_capacity;
        let mut remaining_slice = slice;

        while !remaining_slice.is_empty() {
            let last_chunk = self.get_next_chunk();
            let space_in_chunk = chunk_capacity - last_chunk.len();

            let (slice, rest) = if remaining_slice.len() > space_in_chunk {
                remaining_slice.split_at(space_in_chunk)
            } else {
                (remaining_slice, "")
            };

            remaining_slice = rest;
            last_chunk.push_str(slice);
        }

        self.nbytes += slice_length;
        (start, slice_length)
    }

    /// Returns the last chunk with space available.
    ///
    /// Allocates one more chunk in 2 cases:
    /// - The last chunk has reached `Self::chunk_capacity` limit
    /// - `Self::list_of_chunks` is empty
    fn get_next_chunk(&mut self) -> &mut String {
        let chunk_capacity = self.chunk_capacity;

        if self
            .list_of_chunks
            .last()
            .map(|chunk| {
                debug_assert!(chunk.len() <= chunk_capacity);
                chunk.len() == chunk_capacity
            })
            .unwrap_or(true)
        {
            self.list_of_chunks
                .push(String::with_capacity(self.chunk_capacity));
        }

        // Never fail, we just allocated one in case it's empty
        self.list_of_chunks.last_mut().unwrap()
    }

    pub fn capacity(&self) -> usize {
        self.chunk_capacity * self.list_of_chunks.len()
    }

    pub fn get(&self, Range { start, end }: Range<usize>) -> Option<Cow<str>> {
        let slice_length = end - start;
        let (list_index, chunk_index) = self.get_indexes_at(start);

        let chunk = self.list_of_chunks.get(list_index)?;

        if chunk_index + slice_length <= self.chunk_capacity {
            chunk
                .get(chunk_index..chunk_index + slice_length)
                .map(Cow::Borrowed)
        } else {
            let mut slice = String::with_capacity(slice_length);
            let mut iter_chunk = self.list_of_chunks.get(list_index..)?.iter();
            let mut start_in_chunk = chunk_index;
            let mut length = slice_length;

            while length > 0 {
                let chunk = iter_chunk.next()?;
                let end_in_chunk = (start_in_chunk + length).min(self.chunk_capacity);

                let part_slice = chunk.get(start_in_chunk..end_in_chunk)?;
                slice.push_str(part_slice);

                length -= end_in_chunk - start_in_chunk;
                start_in_chunk = 0;
            }

            debug_assert_eq!(slice.len(), slice_length);

            Some(Cow::Owned(slice))
        }
    }

    fn get_indexes_at(&self, index: usize) -> (usize, usize) {
        let list_index = index / self.chunk_capacity;
        let chunk_index = index % self.chunk_capacity;

        (list_index, chunk_index)
    }

    pub fn len(&self) -> usize {
        self.nbytes
    }

    pub fn deallocate(&mut self) {
        self.list_of_chunks = Vec::new();
        self.nbytes = 0;
    }

    pub fn clear(&mut self) {
        self.list_of_chunks.truncate(1);
        if let Some(first_chunk) = self.list_of_chunks.last_mut() {
            first_chunk.clear();
        };
        self.nbytes = 0;
    }
}
