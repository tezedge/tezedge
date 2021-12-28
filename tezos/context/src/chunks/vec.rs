// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    borrow::Cow,
    ops::{Index, IndexMut, Range, RangeFrom},
    slice::SliceIndex,
};

use super::{mmap::MmappedVec, Chunk, DEFAULT_LIST_LENGTH};

#[derive(Debug)]
enum ChunkEnum<T> {
    InMemory { inner: Vec<T> },
    OnDisk { mmap: MmappedVec<T> },
}

impl<T> std::ops::Deref for ChunkEnum<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        match self {
            ChunkEnum::InMemory { inner } => &inner,
            ChunkEnum::OnDisk { mmap } => &mmap,
        }
    }
}

impl<T> std::ops::DerefMut for ChunkEnum<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            ChunkEnum::InMemory { inner } => inner,
            ChunkEnum::OnDisk { mmap } => mmap,
        }
    }
}

// impl<T> Index<usize> for ChunkEnum<T> {
//     type Output = T;

//     fn index(&self, index: usize) -> &Self::Output {
//         match self {
//             ChunkEnum::InMemory { inner } => &inner[index],
//             ChunkEnum::OnDisk { mmap, .. } => &mmap[index],
//         }
//     }
// }

// impl<T> Index<RangeFrom<usize>> for ChunkEnum<T> {
//     type Output = [T];

//     fn index(&self, range_from: RangeFrom<usize>) -> &Self::Output {
//         match self {
//             ChunkEnum::InMemory { inner } => &inner[range_from],
//             ChunkEnum::OnDisk { mmap, .. } => &mmap[range_from],
//         }
//     }
// }

// impl<T> IndexMut<usize> for ChunkEnum<T> {
//     fn index_mut(&mut self, index: usize) -> &mut Self::Output {
//         match self {
//             ChunkEnum::InMemory { inner } => &mut inner[index],
//             ChunkEnum::OnDisk { mmap, .. } => &mut mmap[index],
//         }
//     }
// }

impl<T> ChunkEnum<T> {
    fn new_in_memory(capacity: usize) -> Self {
        Self::InMemory {
            inner: Vec::with_capacity(capacity),
        }
    }

    fn new_on_disk(capacity: usize) -> Self {
        Self::OnDisk {
            mmap: MmappedVec::with_capacity(capacity),
        }
    }

    fn push(&mut self, elem: T) {
        match self {
            ChunkEnum::InMemory { inner } => inner.push(elem),
            ChunkEnum::OnDisk { mmap } => {
                mmap.push(elem).unwrap();
            }
        }
    }

    // pub fn get<I>(&self, index: I) -> Option<&I::Output>
    // where
    //     I: SliceIndex<Self>,
    // {
    //     match self {
    //         ChunkEnum::InMemory { inner } => inner.get(index),
    //         ChunkEnum::OnDisk { mmap } => todo!(),
    //     }
    // }

    // fn get(&self, index: usize) -> Option<&T> {
    //     match self {
    //         ChunkEnum::InMemory { inner } => inner.get(index),
    //         ChunkEnum::OnDisk { mmap } => mmap.get(index),
    //     }
    // }

    fn len(&self) -> usize {
        match self {
            ChunkEnum::InMemory { inner } => inner.len(),
            ChunkEnum::OnDisk { mmap } => mmap.len(),
        }
    }

    fn capacity(&self) -> usize {
        match self {
            ChunkEnum::InMemory { inner } => inner.capacity(),
            ChunkEnum::OnDisk { mmap } => mmap.capacity(),
        }
    }

    fn clear(&mut self) {
        match self {
            ChunkEnum::InMemory { inner } => inner.clear(),
            ChunkEnum::OnDisk { mmap } => mmap.clear(),
        }
    }

    fn truncate(&mut self, len: usize) {
        match self {
            ChunkEnum::InMemory { inner } => inner.truncate(len),
            ChunkEnum::OnDisk { mmap } => mmap.truncate(len),
        }
    }
}

impl<T: Clone> ChunkEnum<T> {
    fn extend_from_slice(&mut self, slice: &[T]) {
        match self {
            ChunkEnum::InMemory { inner } => inner.extend_from_slice(slice),
            ChunkEnum::OnDisk { mmap } => mmap.extend_from_slice(slice).unwrap(),
        }
    }
}

/// Structure allocating multiple `Chunk`
///
/// Example:
/// ```
/// use tezos_context::chunks::ChunkedVec;
///
/// let mut chunks = ChunkedVec::with_chunk_capacity(1000);
/// let (start, length) = chunks.extend_from_slice(&[1, 2, 3]);
/// assert_eq!(&*chunks.get_slice(start..start + length).unwrap(), &[1, 2, 3]);
/// assert_eq!(*chunks.get(start).unwrap(), 1);
/// ```
#[derive(Debug)]
pub struct ChunkedVec<T> {
    // list_of_chunks: Vec<Chunk<T>>,
    list_of_chunks: Vec<ChunkEnum<T>>,
    chunk_capacity: usize,
    /// Number of elements in the chunks
    nelems: usize,

    nchunks_in_memory: Option<usize>,
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
    /// Extends the last chunk with `slice`
    ///
    /// Return the index of the slice in the chunks, and its length
    pub fn extend_from_slice(&mut self, slice: &[T]) -> (usize, usize) {
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
                (remaining_slice, &[][..])
            };

            remaining_slice = rest;
            last_chunk.extend_from_slice(slice);
        }

        self.nelems += slice_length;
        (start, slice_length)
    }

    /// Appends `other` in the last chunk.
    ///
    /// Return the index of the slice in the chunks, and its length
    pub fn append(&mut self, other: &mut Vec<T>) -> (usize, usize) {
        let (start, length) = self.extend_from_slice(other);
        other.truncate(0);

        (start, length)
    }

    pub fn extend_from(&mut self, other: &Self) {
        let our_length = self.list_of_chunks.len();
        let other_length = other.list_of_chunks.len();

        if our_length != other_length {
            assert!(our_length < other_length);
            self.grow_list_of_chunks(other_length);
            // self.list_of_chunks
            //     .resize_with(other_length, Default::default);
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

        if chunk_index + slice_length <= self.chunk_capacity {
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
                let end_in_chunk = (start_in_chunk + length).min(self.chunk_capacity);

                let part_slice = chunk.get(start_in_chunk..end_in_chunk)?;
                slice.extend_from_slice(part_slice);

                length -= end_in_chunk - start_in_chunk;
                start_in_chunk = 0;
            }

            debug_assert_eq!(slice.len(), slice_length);

            Some(Cow::Owned(slice))
        }
    }

    #[cfg(test)]
    pub fn to_single_vec(&self) -> Vec<T> {
        let cap = self.capacity();
        let mut vec = Vec::with_capacity(cap);

        for chunk in &self.list_of_chunks {
            vec.extend_from_slice(chunk);
        }

        vec
    }
}

impl<T> ChunkedVec<T> {
    /// Returns a new `ChunkedVec<T>` without allocating
    pub fn empty() -> Self {
        Self {
            list_of_chunks: Vec::new(),
            chunk_capacity: 1_000,
            nelems: 0,
            nchunks_in_memory: None,
        }
    }

    pub fn with_chunk_capacity(chunk_capacity: usize) -> Self {
        assert_ne!(chunk_capacity, 0);

        let mut list_of_chunks: Vec<ChunkEnum<T>> = Vec::with_capacity(DEFAULT_LIST_LENGTH);

        let chunk: ChunkEnum<T> = ChunkEnum::new_in_memory(chunk_capacity);
        list_of_chunks.push(chunk);

        Self {
            list_of_chunks,
            chunk_capacity,
            nelems: 0,
            nchunks_in_memory: None,
        }
    }

    pub fn with_chunk_capacity_on_disk(
        mem_chunk_capacity: usize,
        nchunks_in_memory: usize,
    ) -> Self {
        assert_ne!(mem_chunk_capacity, 0);

        let mut list_of_chunks: Vec<ChunkEnum<T>> = Vec::with_capacity(DEFAULT_LIST_LENGTH);

        let chunk = if nchunks_in_memory > 0 {
            ChunkEnum::new_in_memory(mem_chunk_capacity)
        } else {
            ChunkEnum::new_on_disk(mem_chunk_capacity)
        };

        list_of_chunks.push(chunk);

        Self {
            list_of_chunks,
            chunk_capacity: mem_chunk_capacity,
            nelems: 0,
            nchunks_in_memory: Some(nchunks_in_memory),
        }
    }

    pub fn capacity(&self) -> usize {
        self.chunk_capacity * self.list_of_chunks.len()
    }

    /// Returns the last chunk with space available.
    ///
    /// Allocates one more chunk in 2 cases:
    /// - The last chunk has reached `Self::chunk_capacity` limit
    /// - `Self::list_of_chunks` is empty
    fn get_next_chunk(&mut self) -> &mut ChunkEnum<T> {
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
            let on_disk = self
                .nchunks_in_memory
                .as_ref()
                .map(|max| self.list_of_chunks.len() >= *max)
                .unwrap_or(false);

            let chunk = if on_disk {
                ChunkEnum::new_on_disk(self.chunk_capacity)
            } else {
                ChunkEnum::new_in_memory(self.chunk_capacity)
            };

            self.list_of_chunks.push(chunk);
        }

        // Never fail, we just allocated one in case it's empty
        self.list_of_chunks.last_mut().unwrap()
    }

    fn grow_list_of_chunks(&mut self, new_len: usize) {
        let list_length = self.list_of_chunks.len();

        if list_length >= new_len {
            return;
        }

        let chunk_capacity = self.chunk_capacity;

        // TODO: test this
        if let Some(max_in_mem) = self.nchunks_in_memory {
            if new_len > max_in_mem && max_in_mem > list_length {
                self.list_of_chunks
                    .resize_with(max_in_mem, || ChunkEnum::new_in_memory(chunk_capacity));
            }

            self.list_of_chunks
                .resize_with(new_len, || ChunkEnum::new_on_disk(chunk_capacity));
        } else {
            self.list_of_chunks
                .resize_with(new_len, || ChunkEnum::new_in_memory(chunk_capacity));
        }
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
        let list_index = index / self.chunk_capacity;
        let chunk_index = index % self.chunk_capacity;

        (list_index, chunk_index)
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

#[cfg(test)]
mod tests {
    use std::iter::successors;

    use super::*;

    #[test]
    fn test_chunked_without_alloc() {
        let mut chunks = ChunkedVec::with_chunk_capacity(5);

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
        let source_sizes: Vec<_> = successors(Some(1), |n| Some(n + 1)).take(61).collect();
        let chunk_caps = source_sizes.clone();
        let extend_by = &[1, 2, 3, 4];

        for source_size in &source_sizes {
            for chunk_cap in &chunk_caps {
                for extend_by in extend_by {
                    test_chunked_impl(*source_size, *chunk_cap, *extend_by);
                }
            }
        }
    }

    fn test_chunked_impl(source_size: usize, chunk_cap: usize, extend_by: usize) {
        let source: Vec<usize> = successors(Some(0), |n| Some(n + 1))
            .take(source_size)
            .collect();

        let mut chunks = ChunkedVec::with_chunk_capacity_on_disk(chunk_cap, 0);
        // let mut chunks = ChunkedVec::with_chunk_capacity(chunk_cap);

        for i in 0..source_size {
            chunks.push(i);
        }

        let slice = chunks.get_slice(0..source_size).unwrap();
        assert_eq!(slice, source);

        let mut chunks = ChunkedVec::with_chunk_capacity_on_disk(chunk_cap, 0);
        // let mut chunks = ChunkedVec::with_chunk_capacity(chunk_cap);

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
