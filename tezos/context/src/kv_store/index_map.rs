// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    convert::{TryFrom, TryInto},
    marker::PhantomData,
};

use crate::chunks::{ChunkedVec, ChunkedVecIter};

/// A container mapping a typed ID to a value.
///
/// The underlying container is a `Vec` and the id is its index.
#[derive(Debug)]
pub struct IndexMap<K, V, const CHUNK_CAPACITY: usize> {
    pub entries: ChunkedVec<V, CHUNK_CAPACITY>,
    _phantom: PhantomData<K>,
}

impl<K, V, const CHUNK_CAPACITY: usize> Default for IndexMap<K, V, CHUNK_CAPACITY> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V, const CHUNK_CAPACITY: usize> IndexMap<K, V, CHUNK_CAPACITY> {
    pub fn empty() -> Self {
        Self {
            entries: ChunkedVec::empty(),
            _phantom: PhantomData,
        }
    }

    pub fn new() -> Self {
        Self {
            entries: ChunkedVec::default(),
            _phantom: PhantomData,
        }
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

    pub fn get_index(&self, index: usize) -> Option<&V> {
        self.entries.get(index)
    }

    pub fn iter_values(&self) -> impl Iterator<Item = &V> {
        self.entries.iter()
    }

    pub fn iter_with_keys(&self) -> IndexMapIter<'_, K, V, CHUNK_CAPACITY> {
        IndexMapIter {
            chunks: self.entries.iter(),
            _phantom: PhantomData,
        }
    }

    pub fn for_each_mut<F, E>(&mut self, fun: F) -> Result<(), E>
    where
        F: FnMut(&mut V) -> Result<(), E>,
    {
        self.entries.for_each_mut(fun)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Removes the last `nelems` from the chunks.
    pub fn remove_last_nelems(&mut self, nelems: usize) {
        self.entries.remove_last_nelems(nelems);
    }
}

impl<K, V, const CHUNK_CAPACITY: usize> IndexMap<K, V, CHUNK_CAPACITY>
where
    K: TryInto<usize>,
{
    pub fn contains_key(&mut self, key: K) -> Result<bool, K::Error> {
        let index = key.try_into()?;
        Ok(index < self.entries.len())
    }

    pub fn set(&mut self, key: K, value: V) -> Result<V, K::Error> {
        Ok(std::mem::replace(&mut self.entries[key.try_into()?], value))
    }

    pub fn get(&self, key: K) -> Result<Option<&V>, K::Error> {
        Ok(self.entries.get(key.try_into()?))
    }

    pub fn get_mut(&mut self, key: K) -> Result<Option<&mut V>, K::Error> {
        Ok(self.entries.get_mut(key.try_into()?))
    }
}

impl<K, V, const CHUNK_CAPACITY: usize> IndexMap<K, V, CHUNK_CAPACITY>
where
    K: TryFrom<usize>,
{
    pub fn push(&mut self, value: V) -> Result<K, <K as TryFrom<usize>>::Error> {
        let index = self.entries.push(value);
        K::try_from(index)
    }
}

impl<K, V, const CHUNK_CAPACITY: usize> IndexMap<K, V, CHUNK_CAPACITY>
where
    K: TryInto<usize> + Copy,
    K: TryFrom<usize>,
    V: Default,
{
    pub fn get_vacant_entry(&mut self) -> Result<(K, &mut V), <K as TryFrom<usize>>::Error> {
        let index = self.entries.push(Default::default());
        Ok((K::try_from(index)?, &mut self.entries[index]))
    }

    pub fn insert_at(&mut self, key: K, value: V) -> Result<V, <K as TryInto<usize>>::Error> {
        let index: usize = key.try_into()?;

        if index >= self.entries.len() {
            self.entries.resize_with(index + 1, V::default);
        }

        Ok(std::mem::replace(&mut self.entries[index], value))
    }

    pub fn entry(&mut self, key: K) -> Result<&mut V, <K as TryInto<usize>>::Error> {
        if self.contains_key(key)? {
            return Ok(self.get_mut(key)?.unwrap()); // Never fails, `Self::contains_key` returned `true`
        }

        let index: usize = key.try_into()?;
        if index >= self.entries.len() {
            self.entries.resize_with(index + 1, V::default);
        }

        Ok(self.get_mut(key)?.unwrap()) // Never fails, we just resized it
    }
}

pub struct IndexMapIter<'a, K, V, const CHUNK_CAPACITY: usize> {
    chunks: ChunkedVecIter<'a, V, CHUNK_CAPACITY>,
    _phantom: PhantomData<K>,
}

impl<'a, K, V, const CHUNK_CAPACITY: usize> Iterator for IndexMapIter<'a, K, V, CHUNK_CAPACITY>
where
    K: TryFrom<usize>,
{
    type Item = (K, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        let index = self.chunks.index;
        let index: K = index.try_into().ok()?;

        self.chunks.next().map(|v| (index, v))
    }
}
