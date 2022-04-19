// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    borrow::Cow,
    fmt::Debug,
    ops::{Index, IndexMut, Range},
};

use crate::gc::SortedMap;

use super::{Chunk, DEFAULT_LIST_LENGTH};

/// Structure allocating multiple `Chunk`
///
/// Example:
/// ```
/// use tezos_context::chunks::ChunkedVec;
///
/// let mut chunks = ChunkedVec::<_, 1000>::default();
/// let (start, length) = chunks.extend_from_slice(&[1, 2, 3]);
/// assert_eq!(&*chunks.get_slice(start..start + length).unwrap(), &[1, 2, 3]);
/// assert_eq!(*chunks.get(start).unwrap(), 1);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkedVec<T, const CHUNK_CAPACITY: usize> {
    pub list_of_chunks: Vec<Chunk<T>>,
    /// Number of elements in the chunks
    nelems: usize,
}

impl<T, const CHUNK_CAPACITY: usize> Index<usize> for ChunkedVec<T, CHUNK_CAPACITY> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        let (list_index, chunk_index) = self.get_indexes_at(index);

        &self.list_of_chunks[list_index][chunk_index]
    }
}

impl<T, const CHUNK_CAPACITY: usize> IndexMut<usize> for ChunkedVec<T, CHUNK_CAPACITY> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        let (list_index, chunk_index) = self.get_indexes_at(index);

        &mut self.list_of_chunks[list_index][chunk_index]
    }
}

pub struct ChunkedVecIter<'a, T, const CHUNK_CAPACITY: usize> {
    chunks: &'a ChunkedVec<T, CHUNK_CAPACITY>,
    pub index: usize,
}

impl<'a, T, const CHUNK_CAPACITY: usize> Iterator for ChunkedVecIter<'a, T, CHUNK_CAPACITY> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.chunks.get(self.index)?;
        self.index += 1;
        Some(item)
    }
}

impl<T, const CHUNK_CAPACITY: usize> ChunkedVec<T, CHUNK_CAPACITY>
where
    T: Clone,
{
    /// Extends the last chunk with `slice`
    ///
    /// Return the index of the slice in the chunks, and its length
    pub fn extend_from_slice(&mut self, slice: &[T]) -> (usize, usize) {
        let start = self.len();
        let slice_length = slice.len();
        let mut remaining_slice = slice;

        while !remaining_slice.is_empty() {
            let last_chunk = self.get_next_chunk();
            let space_in_chunk = CHUNK_CAPACITY - last_chunk.len();

            let (slice, rest) = if remaining_slice.len() > space_in_chunk {
                remaining_slice.split_at(space_in_chunk)
            } else {
                (remaining_slice, &[][..])
            };

            remaining_slice = rest;
            last_chunk.extend_from_slice(slice);
        }

        self.nelems += slice_length;
        (start, slice_length)
    }

    pub fn extend_from_chunks(&mut self, other: &Self) {
        for chunk in &other.list_of_chunks {
            self.extend_from_slice(chunk);
        }
    }

    /// Appends `other` in the last chunk.
    ///
    /// Return the index of the slice in the chunks, and its length
    pub fn append(&mut self, other: &mut Vec<T>) -> (usize, usize) {
        let (start, length) = self.extend_from_slice(other);
        other.truncate(0);

        (start, length)
    }

    /// Appends `other` chunks.
    pub fn append_chunks<const OTHER_CAP: usize>(&mut self, mut other: ChunkedVec<T, OTHER_CAP>) {
        while let Some(mut chunk) = other.pop_first_chunk() {
            self.append(&mut chunk);
        }
    }

    pub fn deallocate_before(&mut self, index: usize) {
        let (list_index, _) = self.get_indexes_at(index);
        let list_index = list_index.saturating_sub(1);

        let chunks = match self.list_of_chunks.get_mut(0..list_index) {
            Some(chunks) => chunks,
            None => return,
        };

        for chunk in chunks {
            *chunk = Vec::new();
        }
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

        self.nelems += nelems;
        assert_eq!(self.nelems, other.nelems);
    }

    pub fn get_slice(&self, Range { start, end }: Range<usize>) -> Option<Cow<[T]>> {
        let slice_length = end - start;
        let (list_index, chunk_index) = self.get_indexes_at(start);

        let chunk = self.list_of_chunks.get(list_index)?;

        if chunk_index + slice_length <= CHUNK_CAPACITY {
            chunk
                .get(chunk_index..chunk_index + slice_length)
                .map(Cow::Borrowed)
        } else {
            let mut slice = Vec::with_capacity(slice_length);
            let mut iter_chunk = self.list_of_chunks.get(list_index..)?.iter();
            let mut start_in_chunk = chunk_index;
            let mut length = slice_length;

            while length > 0 {
                let chunk = iter_chunk.next()?;
                let end_in_chunk = (start_in_chunk + length).min(CHUNK_CAPACITY);

                let part_slice = chunk.get(start_in_chunk..end_in_chunk)?;
                slice.extend_from_slice(part_slice);

                length -= end_in_chunk - start_in_chunk;
                start_in_chunk = 0;
            }

            debug_assert_eq!(slice.len(), slice_length);

            Some(Cow::Owned(slice))
        }
    }
}

impl<T, const CHUNK_CAPACITY: usize> Default for ChunkedVec<T, CHUNK_CAPACITY> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const CHUNK_CAPACITY: usize> ChunkedVec<T, CHUNK_CAPACITY> {
    /// Returns a new `ChunkedVec<T>` without allocating
    pub fn empty() -> Self {
        Self {
            list_of_chunks: Vec::new(),
            nelems: 0,
        }
    }

    pub fn new() -> Self {
        assert_ne!(CHUNK_CAPACITY, 0);

        let chunk: Vec<T> = Vec::with_capacity(CHUNK_CAPACITY);

        let mut list_of_vec: Vec<Chunk<T>> = Vec::with_capacity(DEFAULT_LIST_LENGTH);
        list_of_vec.push(chunk);

        Self {
            list_of_chunks: list_of_vec,
            nelems: 0,
        }
    }

    pub fn capacity(&self) -> usize {
        CHUNK_CAPACITY * self.list_of_chunks.len()
    }

    /// Returns the last chunk with space available.
    ///
    /// Allocates one more chunk in 2 cases:
    /// - The last chunk has reached `Self::chunk_capacity` limit
    /// - `Self::list_of_chunks` is empty
    fn get_next_chunk(&mut self) -> &mut Chunk<T> {
        let chunk_capacity = CHUNK_CAPACITY;

        let must_alloc_new_chunk = self
            .list_of_chunks
            .last()
            .map(|chunk| {
                debug_assert!(chunk.len() <= chunk_capacity);
                chunk.len() == chunk_capacity
            })
            .unwrap_or(true);

        if must_alloc_new_chunk {
            self.list_of_chunks.push(Vec::with_capacity(CHUNK_CAPACITY));
        }

        // Never fail, we just allocated one in case it's empty
        self.list_of_chunks.last_mut().unwrap()
    }

    pub fn push(&mut self, elem: T) -> usize {
        let index = self.len();
        self.nelems += 1;

        self.get_next_chunk().push(elem);

        index
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
        let list_index = index / CHUNK_CAPACITY;
        let chunk_index = index % CHUNK_CAPACITY;

        (list_index, chunk_index)
    }

    pub fn nchunks(&self) -> usize {
        self.list_of_chunks.len()
    }

    pub fn iter(&self) -> ChunkedVecIter<T, CHUNK_CAPACITY> {
        ChunkedVecIter {
            chunks: self,
            index: 0,
        }
    }

    pub fn iter_from(&self, start: usize) -> ChunkedVecIter<T, CHUNK_CAPACITY> {
        ChunkedVecIter {
            chunks: self,
            index: start,
        }
    }

    // Impossible to make a `Self::iter_mut` without unsafe code:
    // https://stackoverflow.com/q/63437935/5717561
    // So we iterate with a closure instead
    pub fn for_each_mut<F, E>(&mut self, mut fun: F) -> Result<(), E>
    where
        F: FnMut(&mut T) -> Result<(), E>,
    {
        for chunk in &mut self.list_of_chunks {
            for elem in chunk {
                fun(elem)?;
            }
        }

        Ok(())
    }

    pub fn resize_with<F>(&mut self, new_len: usize, mut fun: F)
    where
        F: FnMut() -> T,
    {
        while self.nelems < new_len {
            self.push(fun());
        }
    }

    /// Removes the last `nelems` from the chunks.
    pub fn remove_last_nelems(&mut self, mut nelems: usize) {
        while let Some(last_chunk) = self.list_of_chunks.last_mut() {
            if last_chunk.len() >= nelems {
                self.nelems -= nelems;
                last_chunk.truncate(last_chunk.len() - nelems);
                return;
            } else {
                nelems -= last_chunk.len();
                self.nelems -= last_chunk.len();
                self.list_of_chunks.pop();
            }
        }
    }

    pub fn len(&self) -> usize {
        self.nelems
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
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

    pub fn pop_first_chunk(&mut self) -> Option<Vec<T>> {
        if self.list_of_chunks.is_empty() {
            None
        } else {
            let chunk = self.list_of_chunks.remove(0);
            self.nelems -= chunk.len();
            Some(chunk)
        }
    }

    pub fn pop(&mut self) -> Option<T> {
        let last_chunk = self.list_of_chunks.last_mut()?;
        let last_item = last_chunk.pop()?;

        if last_chunk.is_empty() {
            self.list_of_chunks.pop();
        }

        Some(last_item)
    }

    #[cfg(test)]
    pub fn to_vec(self) -> Vec<T> {
        let mut vec = Vec::with_capacity(self.nelems);
        for mut chunk in self.list_of_chunks.into_iter() {
            vec.append(&mut chunk);
        }
        vec
    }
}

impl<K, V, const CHUNK_CAPACITY: usize> ChunkedVec<(K, V), CHUNK_CAPACITY>
where
    K: Ord + Copy,
{
    pub fn into_sorted_map(&mut self) -> SortedMap<K, V> {
        let mut map = SortedMap::default();

        while !self.list_of_chunks.is_empty() {
            let chunk = self.list_of_chunks.remove(0);
            for (k, v) in chunk.into_iter() {
                map.insert(k, v);
            }
        }

        map.shrink_to_fit();
        map
    }
}

#[cfg(test)]
mod tests {
    use std::iter::successors;

    use super::*;

    #[test]
    fn test_chunked_pop() {
        let mut chunks = ChunkedVec::<_, 2>::default();
        assert!(chunks.pop().is_none());

        chunks.extend_from_slice(&[1, 2, 3, 4, 5, 6, 7]);

        assert_eq!(chunks.pop().unwrap(), 7);
        assert_eq!(chunks.pop().unwrap(), 6);
        assert_eq!(chunks.pop().unwrap(), 5);
        assert_eq!(chunks.pop().unwrap(), 4);
        assert_eq!(chunks.pop().unwrap(), 3);
        assert_eq!(chunks.pop().unwrap(), 2);
        assert_eq!(chunks.pop().unwrap(), 1);
        assert!(chunks.pop().is_none());
    }

    #[test]
    fn test_chunked_pop_chunk() {
        let mut chunks = ChunkedVec::<_, 2>::default();
        assert!(chunks.pop().is_none());
        assert_eq!(chunks.capacity(), 2);

        chunks.extend_from_slice(&[1, 2, 3, 4, 5, 6, 7]);
        assert_eq!(chunks.len(), 7);
        assert_eq!(chunks.capacity(), 8);

        chunks.pop_first_chunk().unwrap();
        assert_eq!(chunks.len(), 5);
        assert_eq!(chunks.get_slice(0..5).unwrap(), &[3, 4, 5, 6, 7][..]);
        assert_eq!(chunks.capacity(), 6);

        chunks.pop_first_chunk().unwrap();
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks.get_slice(0..3).unwrap(), &[5, 6, 7][..]);
        assert_eq!(chunks.capacity(), 4);

        chunks.pop_first_chunk().unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks.get_slice(0..1).unwrap(), &[7][..]);
        assert_eq!(chunks.capacity(), 2);

        chunks.pop_first_chunk().unwrap();
        assert_eq!(chunks.len(), 0);
        assert_eq!(chunks.capacity(), 0);

        assert!(chunks.pop_first_chunk().is_none());
    }

    #[test]
    fn test_chunked_without_alloc() {
        let mut chunks = ChunkedVec::<_, 5>::default();

        chunks.extend_from_slice(&[1, 2, 3, 4, 5, 6, 7]);

        match chunks.get_slice(2..5).unwrap() {
            Cow::Borrowed(s) => assert_eq!(s, &[3, 4, 5]),
            Cow::Owned(_) => panic!("must be borrowed"),
        }

        match chunks.get_slice(3..6).unwrap() {
            Cow::Borrowed(_) => panic!("must be owned"),
            Cow::Owned(s) => assert_eq!(s, &[4, 5, 6]),
        }

        match chunks.get_slice(5..7).unwrap() {
            Cow::Borrowed(s) => assert_eq!(s, &[6, 7]),
            Cow::Owned(_) => panic!("must be borrowed"),
        }
    }

    #[test]
    fn test_chunked() {
        macro_rules! call_test_chunked_impl {
            ($source_size:expr, $extend_by:expr, $($N:tt),*) => ({
                $(test_chunked_impl::<$N>($source_size, $extend_by);)*
            })
        }

        let source_sizes: Vec<_> = successors(Some(1), |n| Some(n + 1)).take(61).collect();
        // let chunk_caps = source_sizes.clone();
        let extend_by = &[1, 2, 3, 4];

        for source_size in &source_sizes {
            for extend_by in extend_by {
                // Call test_chunked_impl with different CHUNK_CAPACITY
                #[rustfmt::skip]
                call_test_chunked_impl!(
                    *source_size, *extend_by,
                    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20,
                    21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37,
                    38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54,
                    55, 56, 57, 58, 59, 60, 61
                );
            }
        }
    }

    fn test_chunked_impl<const CHUNK_CAPACITY: usize>(source_size: usize, extend_by: usize) {
        let source: Vec<usize> = successors(Some(0), |n| Some(n + 1))
            .take(source_size)
            .collect();

        let mut chunks = ChunkedVec::<_, CHUNK_CAPACITY>::default();

        for i in 0..source_size {
            chunks.push(i);
        }

        let slice = chunks.get_slice(0..source_size).unwrap();
        assert_eq!(slice, source);

        let mut chunks = ChunkedVec::<_, CHUNK_CAPACITY>::default();

        for sub_slice in source.chunks(extend_by) {
            chunks.extend_from_slice(sub_slice);
        }

        let slice = chunks.get_slice(0..source_size).unwrap();
        assert_eq!(slice, source);

        for i in 0..source_size {
            assert_eq!(&source[0..i], &*chunks.get_slice(0..i).unwrap());

            assert_eq!(&source[i..], &*chunks.get_slice(i..source_size).unwrap(),);
        }
    }
}
