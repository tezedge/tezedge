// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    convert::{TryFrom, TryInto},
    marker::PhantomData,
};

use crate::chunked_vec::ChunkedVec;

/// A container mapping a typed ID to a value.
///
/// The underlying container is a `Vec` and the id is its index.
#[derive(Debug)]
pub struct IndexMap<K, V> {
    entries: ChunkedVec<V>,
    _phantom: PhantomData<K>,
}

impl<K, V> Default for IndexMap<K, V> {
    fn default() -> Self {
        Self::empty()
    }
}

impl<K, V> IndexMap<K, V> {
    pub fn empty() -> Self {
        Self {
            entries: ChunkedVec::empty(),
            _phantom: PhantomData,
        }
    }

    pub fn with_capacity(cap: usize) -> Self {
        Self {
            entries: ChunkedVec::with_chunk_capacity(cap),
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

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

impl<K, V> IndexMap<K, V>
where
    K: TryInto<usize>,
{
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

impl<K, V> IndexMap<K, V>
where
    K: TryFrom<usize>,
{
    pub fn push(&mut self, value: V) -> Result<K, <K as TryFrom<usize>>::Error> {
        let current = self.entries.len();
        self.entries.push(value);
        K::try_from(current)
    }
}

impl<K, V> IndexMap<K, V>
where
    K: TryInto<usize>,
    K: TryFrom<usize>,
    V: Default,
{
    pub fn get_vacant_entry(&mut self) -> Result<(K, &mut V), <K as TryFrom<usize>>::Error> {
        let current = self.entries.len();
        self.entries.push(Default::default());
        Ok((K::try_from(current)?, &mut self.entries[current]))
    }

    pub fn insert_at(&mut self, key: K, value: V) -> Result<V, <K as TryInto<usize>>::Error> {
        let index: usize = key.try_into()?;

        if index >= self.entries.len() {
            self.entries.resize_with(index + 1, V::default);
        }

        Ok(std::mem::replace(&mut self.entries[index], value))
    }
}
