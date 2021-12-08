use std::ops::{Index, IndexMut};

type Chunk<T> = Vec<T>;

const DEFAULT_LIST_LENGTH: usize = 10;

#[derive(Debug)]
pub struct ChunkedVec<T> {
    list_of_chunks: Vec<Chunk<T>>,
    current_index: usize,
    chunk_capacity: usize,
}

impl<T> Index<usize> for ChunkedVec<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        self.get(index).unwrap()
    }
}

impl<T> IndexMut<usize> for ChunkedVec<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.get_mut(index).unwrap()
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

impl<T> ChunkedVec<T> {
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

        let mut list_of_vec: Vec<Vec<T>> = Vec::with_capacity(DEFAULT_LIST_LENGTH);
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

    pub fn is_empty(&self) -> bool {
        self.current_index == 0
    }

    pub fn capacity(&self) -> usize {
        self.chunk_capacity * self.list_of_chunks.len()
    }

    pub fn push(&mut self, element: T) {
        let list_index = self.current_index / self.chunk_capacity;

        let chunk = match self.list_of_chunks.get_mut(list_index) {
            Some(chunk) => chunk,
            None => {
                self.list_of_chunks
                    .push(Vec::with_capacity(self.chunk_capacity));
                &mut self.list_of_chunks[list_index]
            }
        };

        chunk.push(element);
        self.current_index += 1;
    }

    pub fn len(&self) -> usize {
        self.current_index
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        let list_index = index / self.chunk_capacity;
        let chunk_index = index % self.chunk_capacity;

        self.list_of_chunks
            .get(list_index)
            .and_then(|chunk| chunk.get(chunk_index))
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        let list_index = index / self.chunk_capacity;
        let chunk_index = index % self.chunk_capacity;

        self.list_of_chunks
            .get_mut(list_index)
            .and_then(|chunk| chunk.get_mut(chunk_index))
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
        for (index, chunk) in self.list_of_chunks.iter_mut().enumerate() {
            if index == 0 {
                chunk.clear();
            } else if !chunk.is_empty() {
                *chunk = Vec::new();
            }
        }

        self.current_index = 0;
    }
}
