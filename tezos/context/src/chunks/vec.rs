// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::ops::{Index, IndexMut};

use super::{Chunk, DEFAULT_LIST_LENGTH};

/// Structure allocating multiple `Chunk`, its values are accessible (only) by index
///
/// Example:
/// ```
/// use tezos_context::chunks::ChunkedVec;
///
/// let mut chunks = ChunkedVec::with_chunk_capacity(1000);
/// let a = chunks.push(1);
/// let b = chunks.push(2);
/// assert_eq!(*chunks.get(a).unwrap(), 1);
/// assert_eq!(*chunks.get(b).unwrap(), 2);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkedVec<T> {
    list_of_chunks: Vec<Chunk<T>>,
    /// Index of the last element.
    current_index: usize,
    chunk_capacity: usize,
}

impl<T> Index<usize> for ChunkedVec<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        let (list_index, chunk_index) = self.get_indexes_at(index);

        &self.list_of_chunks[list_index][chunk_index]
    }
}

impl<T> IndexMut<usize> for ChunkedVec<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        let (list_index, chunk_index) = self.get_indexes_at(index);

        &mut self.list_of_chunks[list_index][chunk_index]
    }
}

pub struct ChunkedVecIter<'a, T> {
    chunks: &'a ChunkedVec<T>,
    index: usize,
}

impl<'a, T> Iterator for ChunkedVecIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.chunks.get(self.index)?;
        self.index += 1;
        Some(item)
    }
}

impl<T> ChunkedVec<T>
where
    T: Clone,
{
    pub fn extend_from(&mut self, other: &Self) {
        let our_length = self.list_of_chunks.len();
        let other_length = other.list_of_chunks.len();

        if our_length != other_length {
            assert!(our_length < other_length);
            self.list_of_chunks
                .resize_with(other_length, Default::default);
        }

        let our_length = our_length.saturating_sub(1);
        let mut nelems = 0;

        for (ours, other) in self.list_of_chunks[our_length..]
            .iter_mut()
            .zip(&other.list_of_chunks[our_length..])
        {
            let ours_length = ours.len();
            if ours_length < other.len() {
                nelems += other.len() - ours_length;
                ours.extend_from_slice(&other[ours_length..]);
            }
        }

        self.current_index += nelems;
        assert_eq!(self.current_index, other.current_index);
    }
}

impl<T> ChunkedVec<T> {
    /// Returns a new `ChunkedVec<T>` without allocating
    pub fn empty() -> Self {
        Self {
            list_of_chunks: Vec::new(),
            current_index: 0,
            chunk_capacity: 1_000,
        }
    }

    pub fn with_chunk_capacity(chunk_capacity: usize) -> Self {
        assert_ne!(chunk_capacity, 0);

        let chunk: Vec<T> = Vec::with_capacity(chunk_capacity);

        let mut list_of_vec: Vec<Chunk<T>> = Vec::with_capacity(DEFAULT_LIST_LENGTH);
        list_of_vec.push(chunk);

        Self {
            list_of_chunks: list_of_vec,
            current_index: 0,
            chunk_capacity,
        }
    }

    pub fn iter(&self) -> ChunkedVecIter<T> {
        ChunkedVecIter {
            chunks: self,
            index: 0,
        }
    }

    pub fn iter_from(&self, start: usize) -> ChunkedVecIter<T> {
        ChunkedVecIter {
            chunks: self,
            index: start,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.current_index == 0
    }

    pub fn capacity(&self) -> usize {
        self.chunk_capacity * self.list_of_chunks.len()
    }

    /// Push an element in the last chunk.
    ///
    /// Return the index of the new element.
    pub fn push(&mut self, element: T) -> usize {
        let list_index = self.current_index / self.chunk_capacity;

        let chunk = match self.list_of_chunks.get_mut(list_index) {
            Some(chunk) => chunk,
            None => {
                self.list_of_chunks
                    .push(Vec::with_capacity(self.chunk_capacity));
                &mut self.list_of_chunks[list_index]
            }
        };

        let index = self.current_index;

        chunk.push(element);
        self.current_index += 1;

        index
    }

    pub fn len(&self) -> usize {
        self.current_index
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        let (list_index, chunk_index) = self.get_indexes_at(index);

        self.list_of_chunks.get(list_index)?.get(chunk_index)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        let (list_index, chunk_index) = self.get_indexes_at(index);

        self.list_of_chunks
            .get_mut(list_index)?
            .get_mut(chunk_index)
    }

    fn get_indexes_at(&self, index: usize) -> (usize, usize) {
        let list_index = index / self.chunk_capacity;
        let chunk_index = index % self.chunk_capacity;

        (list_index, chunk_index)
    }

    pub fn resize_with<F>(&mut self, new_len: usize, mut fun: F)
    where
        F: FnMut() -> T,
    {
        while self.current_index < new_len {
            self.push(fun());
        }
    }

    pub fn deallocate(&mut self) {
        self.list_of_chunks = Vec::new();
        self.current_index = 0;
    }

    pub fn clear(&mut self) {
        self.list_of_chunks.truncate(1);
        if let Some(first_chunk) = self.list_of_chunks.last_mut() {
            first_chunk.clear();
        };
        self.current_index = 0;
    }
}
