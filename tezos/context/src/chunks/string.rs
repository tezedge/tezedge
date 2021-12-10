use std::ops::{Index, Range};

use super::DEFAULT_LIST_LENGTH;

/// Structure similar to `ChunkedSlice` but using `String` instead of `Vec<T>`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkedString {
    list_of_chunks: Vec<String>,
    chunk_capacity: usize,
    /// Number of bytes
    nbytes: usize,
}

impl Index<Range<usize>> for ChunkedString {
    type Output = str;

    fn index(&self, Range { start, end }: Range<usize>) -> &Self::Output {
        let length = end - start;
        let (list_index, chunk_index) = self.get_indexes_at(start);

        &self.list_of_chunks[list_index][chunk_index..chunk_index + length]
    }
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
        self.maybe_alloc_chunk();

        let start = self.get_start();
        let slice_length = slice.len();
        self.nbytes += slice_length;

        self.list_of_chunks
            .last_mut()
            .unwrap() // Never fail, we called `Self::maybe_alloc_chunk`
            .push_str(slice);

        (start, slice_length)
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
                .push(String::with_capacity(self.chunk_capacity));
        }
    }

    pub fn get(&self, Range { start, end }: Range<usize>) -> Option<&str> {
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
            self.nbytes -= nelems;
        };
    }

    /// Returns the index of the next slice to be pushed in the chunks.
    ///
    /// This must be called after any `Self::maybe_alloc_chunk`.
    fn get_start(&self) -> usize {
        let nfull_chunk = self.list_of_chunks.len().saturating_sub(1);
        let last_chunk_length = self.list_of_chunks.last().map(String::len).unwrap_or(0);

        (nfull_chunk * self.chunk_capacity) + last_chunk_length
    }

    pub fn nbytes(&self) -> usize {
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
