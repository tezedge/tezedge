use std::ops::{Index, Range, RangeFrom};

type Chunk<T> = Vec<T>;

const DEFAULT_LIST_LENGTH: usize = 10;

#[derive(Debug)]
pub struct ChunkedSlice<T> {
    list_of_chunks: Vec<Chunk<T>>,
    // current_index: usize,
    chunk_capacity: usize,
}

impl<T> Index<Range<usize>> for ChunkedSlice<T> {
    type Output = [T];

    fn index(&self, Range { start, end }: Range<usize>) -> &Self::Output {
        let length = end - start;
        let (list_index, chunk_index) = self.get_indexes_at(start);

        &self.list_of_chunks[list_index][chunk_index..chunk_index + length]
    }
}

impl<T> Index<RangeFrom<usize>> for ChunkedSlice<T> {
    type Output = [T];

    fn index(&self, RangeFrom { start }: RangeFrom<usize>) -> &Self::Output {
        let (list_index, chunk_index) = self.get_indexes_at(start);

        &self.list_of_chunks[list_index][chunk_index..]
    }
}

// impl<T> IndexMut<usize> for ChunkedVec<T> {
//     fn index_mut(&mut self, index: usize) -> &mut Self::Output {
//         let list_index = index / self.chunk_capacity;
//         let chunk_index = index % self.chunk_capacity;

//         &mut self.list_of_chunks[list_index][chunk_index]
//     }
// }

impl<T> ChunkedSlice<T> {
    pub fn empty() -> Self {
        Self {
            list_of_chunks: Vec::new(),
            // current_index: 0,
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
            // current_index: 0,
            chunk_capacity,
        }
    }

    pub fn capacity(&self) -> usize {
        self.chunk_capacity * self.list_of_chunks.len()
    }

    pub fn append(&mut self, other: &mut Vec<T>) -> (usize, usize) {
        let other_length = other.len();

        match self.list_of_chunks.last_mut() {
            Some(chunk) if chunk.len() >= self.chunk_capacity => {
                self.list_of_chunks
                    .push(Vec::with_capacity(self.chunk_capacity));
                // self.list_of_chunks.last_mut().unwrap()
            }
            None => {
                self.list_of_chunks
                    .push(Vec::with_capacity(self.chunk_capacity));
                // self.list_of_chunks.last_mut().unwrap()
            }
            Some(_) => (),
        };

        let start = self.len();

        self.list_of_chunks.last_mut().unwrap().append(other);

        (start, other_length)

        // let (list_index, chunk_index) = self.get_indexes_at(self.current_index);

        // let chunk = match self.list_of_chunks.get_mut(list_index) {
        //     Some(chunk) => chunk,
        //     None => {
        //         self.list_of_chunks
        //             .push(Vec::with_capacity(self.chunk_capacity));
        //         &mut self.list_of_chunks[list_index]
        //     }
        // };

        // chunk.append(other);
        // self.current_index += other_length;
    }

    // pub fn append(&mut self, other: &mut Vec<T>) {
    //     let other_length = other.len();
    //     let (list_index, chunk_index) = self.get_indexes_at(self.current_index);

    //     let chunk = match self.list_of_chunks.get_mut(list_index) {
    //         Some(chunk) => chunk,
    //         None => {
    //             self.list_of_chunks
    //                 .push(Vec::with_capacity(self.chunk_capacity));
    //             &mut self.list_of_chunks[list_index]
    //         }
    //     };

    //     chunk.append(other);
    //     self.current_index += other_length;
    // }

    pub fn get_slice(&self, Range { start, end }: Range<usize>) -> Option<&[T]> {
        let length = end - start;
        let (list_index, chunk_index) = self.get_indexes_at(start);

        // let aa: Vec<_> = self.list_of_chunks.iter().map(|c| c.len()).collect();
        // println!(
        //     "GET_SLICE Range={:?} list_index={:?} chunk_index={:?} length={:?} end={:?} list={:?}",
        //     Range { start, end }, list_index, chunk_index, length, chunk_index + length, aa
        // );

        self.list_of_chunks
            .get(list_index)?
            .get(chunk_index..chunk_index + length)
    }

    pub fn get_mut_at(&mut self, start: usize, offset: usize) -> &mut T {
        // println!("GET_MUT_AT CALLED");
        let (list_index, chunk_index) = self.get_indexes_at(start);
        &mut self.list_of_chunks[list_index][chunk_index + offset]
    }

    pub fn insert_at(&mut self, start: usize, offset: usize, elem: T) {
        // println!("INSERT_AT CALLED");
        let (list_index, chunk_index) = self.get_indexes_at(start);

        let chunk = &mut self.list_of_chunks[list_index];
        chunk.insert(chunk_index + offset, elem);
    }

    fn get_indexes_at(&self, index: usize) -> (usize, usize) {
        let list_index = index / self.chunk_capacity;
        let chunk_index = index % self.chunk_capacity;

        (list_index, chunk_index)
    }

    pub fn remove_last_nelems(&mut self, nelems: usize) {
        let chunk = match self.list_of_chunks.last_mut() {
            Some(chunk) => chunk,
            None => return,
        };

        chunk.truncate(chunk.len() - nelems);
    }

    // pub fn truncate(&mut self, new_len: usize) {
    // println!("TRUNCATE CALLED");
    // if new_len >= self.current_index {
    //     return;
    // }

    // let (list_index, chunk_index) = self.get_indexes_at(new_len);

    // let first_chunk = match self.list_of_chunks.get_mut(list_index) {
    //     Some(chunk) => chunk,
    //     None => return,
    // };

    // first_chunk.truncate(chunk_index);
    // self.current_index = new_len;

    // let rest_chunks = match self.list_of_chunks.get_mut(list_index + 1..) {
    //     Some(chunks) => chunks,
    //     None => return,
    // };

    // for chunk in rest_chunks {
    //     chunk.truncate(0);
    // }
    // }

    pub fn len(&self) -> usize {
        let length = ((self.list_of_chunks.len() - 1) * self.chunk_capacity)
            + self.list_of_chunks.last().map(|c| c.len()).unwrap_or(0);

        // let list: Vec<_> = self.list_of_chunks.iter().map(|c| c.len()).collect();
        // println!("LENGTH={:?} LIST={:?}", length, list);

        length

        // self.current_index
    }

    pub fn deallocate(&mut self) {
        self.list_of_chunks = Vec::new();
        // self.current_index = 0;
    }

    pub fn clear(&mut self) {
        for (index, chunk) in self.list_of_chunks.iter_mut().enumerate() {
            if index == 0 {
                chunk.clear();
            } else if !chunk.is_empty() {
                *chunk = Vec::new();
            }
        }

        // self.current_index = 0;
    }
}

// struct GrowableSlice<'a, T> {
//     slice: &'a mut Vec<T>,
//     start: usize,
// }
