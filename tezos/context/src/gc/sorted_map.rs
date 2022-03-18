// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

/// Container of key-value pair
///
/// The key must be `Ord` and `Clone`
/// It is intended to reduce the memory usage, compared to `BTreeMap` and `HashMap`
/// It is also, sometimes, faster than `BTreeMap` and `HashMap`.
///
/// Performance / memory usage with `100_000_000` elements
///
/// `SortedMap`: 769 MB
/// `HashMap`:  1154 MB
/// `BTreeMap`: 1999 MB
///
/// SortedMap:
///  insert: 5.785713747s
///  search: 3.751386295s
///  remove: 8.74024244s
/// BTreeMap:
///  insert: 7.825834568s
///  search: 5.327495057s
///  remove: 3.362372303s
/// HashMap (with the default hasher):
///  insert: 13.462065165s
///  search: 10.665816797s
///  remove: 16.224933928s
///
/// This can be reproduced with the test `compare_sorted_map` below
///
use std::{fmt::Debug, sync::Arc};

use static_assertions::assert_eq_size;

use crate::kv_store::HashId;

/// This affects both performance and memory usage
/// 1_000 seems to be a good value.
pub const DEFAULT_CHUNK_SIZE: usize = 1_000;

const fn const_split_at(chunk_size: usize) -> usize {
    (chunk_size / 2) + (chunk_size / 4)
}

#[derive(Clone)]
struct Chunk<K, V, const CHUNK_SIZE: usize>
where
    K: Ord,
{
    inner: Vec<(K, V)>,
    min: K,
    max: K,
}

assert_eq_size!([u8; 40], Chunk<HashId, Arc<[u8]>, 1>);
assert_eq_size!([u8; 24], (HashId, Arc<[u8]>));
assert_eq_size!([u8; 48], [(HashId, Arc<[u8]>); 2]);

// TODO: Use const default when the feature is stabilized, it will be in 1.59.0:
// https://github.com/rust-lang/rust/pull/90207
#[derive(Clone)]
pub struct SortedMap<K, V, const CHUNK_SIZE: usize>
where
    K: Ord,
{
    list: Vec<Chunk<K, V, CHUNK_SIZE>>,
}

impl<K, V, const CHUNK_SIZE: usize> Default for SortedMap<K, V, CHUNK_SIZE>
where
    K: Ord,
{
    fn default() -> Self {
        Self {
            list: Default::default(),
        }
    }
}

impl<K, V, const CHUNK_SIZE: usize> std::fmt::Debug for Chunk<K, V, CHUNK_SIZE>
where
    K: Ord + std::fmt::Debug,
    V: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Chunk")
            .field("inner", &self.inner)
            .field("inner_cap", &self.inner.capacity())
            .field("min", &self.min)
            .field("max", &self.max)
            .finish()
    }
}

impl<K, V, const CHUNK_SIZE: usize> std::fmt::Debug for SortedMap<K, V, CHUNK_SIZE>
where
    K: Ord + std::fmt::Debug,
    V: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut len = 0;
        let mut cap = 0;

        for chunk in self.list.iter() {
            len += chunk.inner.len();
            cap += chunk.inner.capacity();
        }

        f.debug_struct("SortedMap")
            // .field("chunks", &self.list)
            .field("nchunks", &self.list.len())
            .field("list_len", &len)
            .field("list_cap", &cap)
            .finish()
    }
}

pub enum VacantEntry<'a, K, V, const CHUNK_SIZE: usize>
where
    K: Ord,
{
    ListIndex {
        map: &'a mut SortedMap<K, V, CHUNK_SIZE>,
        list_index: usize,
        key: K,
    },
    ListChunkIndexes {
        map: &'a mut SortedMap<K, V, CHUNK_SIZE>,
        list_index: usize,
        chunk_index: usize,
        key: K,
    },
}

impl<'a, K, V, const CHUNK_SIZE: usize> VacantEntry<'a, K, V, CHUNK_SIZE>
where
    K: Ord + Copy,
{
    pub fn insert(self, value: V) {
        match self {
            VacantEntry::ListIndex {
                map,
                list_index,
                key,
            } => map.insert_impl(key, value, Err(list_index), None),
            VacantEntry::ListChunkIndexes {
                map,
                list_index,
                chunk_index,
                key,
            } => map.insert_impl(key, value, Ok(list_index), Some(Err(chunk_index))),
        }
    }
}

pub enum Entry<'a, K, V, const CHUNK_SIZE: usize>
where
    K: Ord,
{
    Occupied(&'a mut V),
    Vacant(VacantEntry<'a, K, V, CHUNK_SIZE>),
}

impl<'a, K, V, const CHUNK_SIZE: usize> std::fmt::Debug for Entry<'a, K, V, CHUNK_SIZE>
where
    K: Ord + Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Occupied(_) => f.write_str("Occupied"),
            Self::Vacant(VacantEntry::ListIndex { list_index, .. }) => f
                .debug_struct("Vacant::ListIndex")
                .field("list_index", list_index)
                .finish(),
            Self::Vacant(VacantEntry::ListChunkIndexes {
                chunk_index,
                list_index,
                ..
            }) => f
                .debug_struct("Vacant::ListChunkIndexes")
                .field("list_index", list_index)
                .field("chunk_index", chunk_index)
                .finish(),
        }
    }
}

impl<K, V, const CHUNK_SIZE: usize> Chunk<K, V, CHUNK_SIZE>
where
    K: Ord + Copy,
{
    fn shrink_to_fit(&mut self) {
        self.inner.shrink_to_fit();
    }

    fn len(&self) -> usize {
        self.inner.len()
    }

    fn capacity(&self) -> usize {
        self.inner.capacity()
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn new_with_element(key: K, value: V) -> Self {
        let mut inner = Vec::with_capacity(CHUNK_SIZE / 2);

        let min = key;
        let max = key;

        inner.push((key, value));

        Self { inner, min, max }
    }

    fn set_min_max(&mut self) {
        if let Some(first) = self.inner.first() {
            self.min = first.0;
        };
        if let Some(last) = self.inner.last() {
            self.max = last.0;
        };
    }

    fn get(&self, key: &K) -> Option<&V> {
        let index = self.inner.binary_search_by_key(key, |&(k, _)| k).ok()?;
        self.inner.get(index).map(|(_, v)| v)
    }

    fn append(&mut self, other: &mut Self) {
        self.inner.append(&mut other.inner);
        if let Some(last) = self.inner.last() {
            self.max = last.0;
        };
    }

    fn remove(&mut self, key: &K) -> Option<V> {
        let index = self.inner.binary_search_by_key(key, |&(k, _)| k).ok()?;
        let item = self.inner.remove(index);

        Some(item.1)
    }

    fn index_in_chunk(&self, key: &K) -> Result<usize, usize> {
        self.inner.binary_search_by_key(key, |&(k, _)| k)
    }

    fn insert_impl(&mut self, key: K, value: V, search: Result<usize, usize>) -> Option<Self> {
        let insert_at = match search {
            Ok(index) => {
                self.inner[index].1 = value;
                return None;
            }
            Err(index) => index,
        };

        let mut next_chunk = None;

        if self.len() == CHUNK_SIZE {
            // The chunk is full, split it in 2 chunks

            let const_split_at = const_split_at(CHUNK_SIZE);

            let new_chunk = if insert_at >= const_split_at {
                let mut new_chunk = self.inner.split_off(const_split_at);
                new_chunk.insert(insert_at - const_split_at, (key, value));
                new_chunk
            } else {
                let new_chunk = self.inner.split_off(const_split_at - 1);
                self.inner.insert(insert_at, (key, value));
                new_chunk
            };

            let new_min = new_chunk[0].0;
            let new_max = new_chunk.last().unwrap().0; // Never fail, new_chunk is not empty

            self.max = self.inner.last().unwrap().0; // Never fail, self is not empty

            next_chunk = Some(Chunk {
                inner: new_chunk,
                min: new_min,
                max: new_max,
            });
        } else {
            self.inner.insert(insert_at, (key, value));

            if key < self.min {
                self.min = key;
            }

            if key > self.max {
                self.max = key;
            }
        }

        next_chunk
    }

    fn insert(&mut self, key: K, value: V) -> Option<Self> {
        self.insert_impl(key, value, self.index_in_chunk(&key))
    }
}

impl<K, V, const CHUNK_SIZE: usize> SortedMap<K, V, CHUNK_SIZE>
where
    K: Ord + Copy,
{
    pub fn shrink_to_fit(&mut self) {
        for chunk in self.list.iter_mut() {
            chunk.shrink_to_fit();
        }
    }

    pub fn contains_key(&self, key: &K) -> bool {
        self.get(key).is_some()
    }

    pub fn keys_to_vec(mut self) -> Vec<K> {
        let mut vec = Vec::with_capacity(self.len());

        while let Some(chunk) = self.list.get(0) {
            for (key, _) in &chunk.inner {
                vec.push(*key);
            }
            self.list.remove(0);
        }

        vec
    }

    pub fn len(&self) -> usize {
        self.list.iter().fold(0, |acc, c| acc + c.len())
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        let index = self.binary_search(key).ok()?;
        let chunk = self.list.get(index)?;

        chunk.get(key)
    }

    pub fn remove(&mut self, key: &K) -> Option<V> {
        let index = self.binary_search(key).ok()?;

        let chunk = &mut self.list[index];
        let item = chunk.remove(key);

        if chunk.is_empty() {
            self.list.remove(index);
        } else {
            chunk.set_min_max();
        }

        item
    }

    fn insert_impl(
        &mut self,
        key: K,
        value: V,
        list_search: Result<usize, usize>,
        chunk_search: Option<Result<usize, usize>>,
    ) {
        match list_search {
            Ok(index) => {
                let chunk = &mut self.list[index];

                let chunk_search = match chunk_search {
                    Some(chunk_search) => chunk_search,
                    None => chunk.index_in_chunk(&key),
                };

                let mut new_chunk = match chunk.insert_impl(key, value, chunk_search) {
                    Some(new_chunk) => new_chunk,
                    None => return,
                };

                let can_merge_with_next = self
                    .list
                    .get(index + 1)
                    .map(|c| c.len() + new_chunk.len() < CHUNK_SIZE)
                    .unwrap_or(false);

                if can_merge_with_next {
                    new_chunk.append(self.list.get_mut(index + 1).unwrap()); // Never fail, can_merge_with_next is `true`
                    self.list[index + 1] = new_chunk
                } else {
                    self.list.insert(index + 1, new_chunk);
                }
            }
            Err(index) => {
                // We first attempt to insert in the previous chunk
                if let Some(prev) = index.checked_sub(1) {
                    // Length of the next chunk
                    let next_size = self.list.get(index).map(|c| c.len());

                    let prev_chunk = &mut self.list[prev];

                    // Does the next chunk has more space available ?
                    let next_is_less_busy = next_size
                        .map(|next_size| next_size < prev_chunk.len())
                        .unwrap_or(false);

                    if !next_is_less_busy && prev_chunk.len() < CHUNK_SIZE {
                        let new = prev_chunk.insert(key, value);
                        debug_assert!(new.is_none());
                        return;
                    }
                };

                // Are we at the end of the list ?
                if self.list.len() == index {
                    self.list.push(Chunk::new_with_element(key, value));
                    return;
                }

                let chunk = &mut self.list[index];

                if chunk.len() < CHUNK_SIZE {
                    // The next chunk has some space available, insert it our new elemet
                    let new = chunk.insert(key, value);
                    debug_assert!(new.is_none());
                    return;
                }

                // Neither the previous or next chunks have spaces, insert a new chunk in
                // the middle
                let chunk = Chunk::new_with_element(key, value);
                self.list.insert(index, chunk);
            }
        }
    }

    pub fn insert(&mut self, key: K, value: V) {
        self.insert_impl(key, value, self.binary_search(&key), None)
    }

    fn binary_search(&self, key: &K) -> Result<usize, usize> {
        let key = *key;

        self.list.binary_search_by(|chunk| {
            if chunk.min > key {
                std::cmp::Ordering::Greater
            } else if chunk.max < key {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        })
    }

    pub fn total_bytes(&self) -> usize {
        let list_size = self.list.len() * std::mem::size_of::<Chunk<K, V, CHUNK_SIZE>>();
        // TODO: Change this
        let capacity = self.list.iter().fold(0, |acc, c| acc + c.capacity());

        list_size + (capacity * std::mem::size_of::<(K, V)>())
    }

    pub fn entry(&mut self, key: K) -> Entry<K, V, CHUNK_SIZE> {
        let list_index = match self.binary_search(&key) {
            Ok(list_index) => list_index,
            Err(list_index) => {
                return Entry::Vacant(VacantEntry::ListIndex {
                    map: self,
                    list_index,
                    key,
                });
            }
        };

        match self.list[list_index].index_in_chunk(&key) {
            Ok(chunk_index) => Entry::Occupied(&mut self.list[list_index].inner[chunk_index].1),
            Err(chunk_index) => Entry::Vacant(VacantEntry::ListChunkIndexes {
                map: self,
                list_index,
                chunk_index,
                key,
            }),
        }
    }
}

impl<K, V, const CHUNK_SIZE: usize> SortedMap<K, V, CHUNK_SIZE>
where
    K: Ord + Copy + Debug,
    V: Debug,
{
    /// [test-only] Make sure that the map is correctly ordered
    #[cfg(test)]
    fn assert_correct(&self) {
        let mut prev_chunk = Option::<(K, K)>::None;

        for chunk in &self.list {
            if let Some(prev_chunk) = prev_chunk {
                assert!(prev_chunk.0 < chunk.min);
                assert!(prev_chunk.1 < chunk.max);
                assert!(prev_chunk.1 < chunk.min);
            };

            assert!(chunk.min <= chunk.max);
            assert_eq!(chunk.min, chunk.inner[0].0);
            assert_eq!(chunk.max, chunk.inner.last().unwrap().0);

            let mut prev = None;
            for item in &chunk.inner {
                if let Some(prev) = prev {
                    assert!(prev < item.0);
                }
                prev = Some(item.0);
            }

            prev_chunk = Some((chunk.min, chunk.max));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashMap};

    use super::*;

    #[test]
    fn test_sorted_map() {
        let mut map = SortedMap::<_, _, 4>::default();

        map.insert(101, 101);
        map.insert(110, 110);
        map.insert(100, 100);
        map.insert(100, 100);
        map.insert(102, 102);

        assert_eq!(map.list.len(), 1);

        map.insert(104, 104);
        assert_eq!(map.list.len(), 2);

        map.insert(114, 114);
        map.insert(200, 200);
        assert_eq!(map.list.len(), 2);
        map.insert(201, 201);
        assert_eq!(map.list.len(), 3);
        map.insert(120, 120);
        map.insert(119, 119);
        map.insert(115, 115);
        map.insert(116, 116);

        map.insert(117, 117);
        map.insert(117, 1117);

        assert_eq!(map.list.len(), 4);

        map.insert(99, 99);
        assert_eq!(map.list.len(), 4);
        map.insert(98, 98);
        assert_eq!(map.list.len(), 5);

        println!("MAP={:#?}", map);

        map.assert_correct();

        map.remove(&200);
        map.remove(&120);
        assert_eq!(map.list.len(), 5);
        map.assert_correct();
        map.remove(&201);
        assert_eq!(map.list.len(), 4);

        map.remove(&98);
        map.assert_correct();
        assert_eq!(map.list.len(), 3);

        map.remove(&104);
        map.remove(&110);
        map.assert_correct();
        assert_eq!(map.list.len(), 3);
        map.remove(&114);
        assert_eq!(map.list.len(), 2);

        println!("MAP={:#?}", map);

        map.assert_correct();
    }

    #[test]
    fn test_sorted_map_big() {
        let mut map = SortedMap::<_, _, 4>::default();

        for index in 0..100_000 {
            map.insert(index, index);
        }
        map.assert_correct();

        assert_eq!(map.list.len(), 25_000);
    }

    #[test]
    fn test_sorted_map_big_revert() {
        let mut map = SortedMap::<_, _, 4>::default();

        for index in 0..100_000 {
            map.insert(100_000 - index, index);
        }
        map.assert_correct();

        assert_eq!(map.list.len(), 25_000);
    }

    #[test]
    fn test_sorted_map_big_entry() {
        let mut map = SortedMap::<_, _, 4>::default();

        for index in 0..100_000 {
            match map.entry(index) {
                Entry::Occupied(_) => panic!(),
                Entry::Vacant(entry) => {
                    entry.insert(index + 1);
                }
            }
        }
        map.assert_correct();

        for index in 0..100_000 {
            match map.entry(index) {
                Entry::Occupied(v) => assert_eq!(*v, index + 1),
                Entry::Vacant(_) => panic!(),
            }
        }
        map.assert_correct();

        assert_eq!(map.list.len(), 25_000);
    }

    #[test]
    fn test_sorted_map_entry() {
        let mut map = SortedMap::<_, _, 4>::default();

        const N: usize = 100_000;

        for index in 0..N {
            if index % 2 == 0 {
                continue;
            }
            match map.entry(index) {
                Entry::Occupied(_) => panic!(),
                Entry::Vacant(entry) => {
                    entry.insert(index + 1);
                }
            }
        }
        map.assert_correct();

        for index in 0..N {
            if index % 2 != 0 {
                continue;
            }
            match map.entry(index) {
                Entry::Occupied(_) => panic!(),
                Entry::Vacant(entry) => {
                    entry.insert(index + 1);
                }
            }
        }
        map.assert_correct();

        for index in 0..N {
            match map.entry(index) {
                Entry::Occupied(v) => assert_eq!(*v, index + 1),
                Entry::Vacant(_) => panic!(),
            }
        }
        map.assert_correct();

        assert_eq!(map.list.len(), 37_500);
    }

    #[test]
    fn test_sorted_map_big_revert_entry() {
        let mut map = SortedMap::<_, _, 4>::default();

        for index in 0..100_000 {
            match map.entry(100_000 - index) {
                Entry::Occupied(_) => panic!(),
                Entry::Vacant(entry) => {
                    entry.insert(index + 1);
                }
            }
        }
        map.assert_correct();

        for index in 0..100_000 {
            match map.entry(100_000 - index) {
                Entry::Occupied(v) => assert_eq!(*v, index + 1),
                Entry::Vacant(_) => panic!(),
            }
        }
        map.assert_correct();

        assert_eq!(map.list.len(), 25_000);
    }

    #[test]
    fn compare_sorted_map() {
        macro_rules! impl_container {
            ($container:ty) => {{
                impl<K, V> Container<K, V> for $container
                where
                    K: Ord + Copy,
                    K: std::hash::Hash,
                {
                    fn insert(&mut self, key: K, value: V) {
                        self.insert(key, value);
                    }
                    fn get(&self, key: &K) -> Option<&V> {
                        self.get(key)
                    }
                    fn remove(&mut self, key: &K) -> Option<V> {
                        self.remove(key)
                    }
                }
            }};
        }

        trait Container<K, V>: Default {
            fn insert(&mut self, key: K, value: V);
            fn get(&self, key: &K) -> Option<&V>;
            fn remove(&mut self, key: &K) -> Option<V>;
        }

        impl_container!(SortedMap<K, V, DEFAULT_CHUNK_SIZE>);
        impl_container!(BTreeMap<K, V>);
        impl_container!(HashMap<K, V>);

        const NUMBER_OF_INSERT: i32 = 1_000_000;
        // const NUMBER_OF_INSERT: i32 = 100_000_000;

        fn run<T>()
        where
            T: Container<i32, i32>,
        {
            let mut map = T::default();

            let now = std::time::Instant::now();
            for index in 0..NUMBER_OF_INSERT {
                if index % 2 == 0 {
                    map.insert(index, index);
                }
            }
            for index in 0..NUMBER_OF_INSERT {
                // This force our `SortedMap` to reallocate:
                // Make it use the "worse case" path
                if index % 2 != 0 {
                    map.insert(index, index);
                }
            }
            println!(" Insert: {:?}", now.elapsed());

            let now = std::time::Instant::now();
            for index in 0..NUMBER_OF_INSERT {
                assert!(map.get(&index).is_some());
            }
            println!(" Search: {:?}", now.elapsed());

            let now = std::time::Instant::now();
            for index in 0..NUMBER_OF_INSERT {
                assert!(map.remove(&index).is_some());
            }
            println!(" Remove: {:?}", now.elapsed());
        }

        println!("SortedMap:");
        run::<SortedMap<_, _, DEFAULT_CHUNK_SIZE>>();

        println!("BTreeMap:");
        run::<BTreeMap<_, _>>();

        println!("HashMap:");
        run::<HashMap<_, _>>();
    }
}
