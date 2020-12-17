// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::{BinaryHeap, HashSet};

use crypto::hash::BlockHash;

/// Data structure for holding data about unique block collections
pub(crate) struct UniqueBlockData<T> {
    binary_heap: BinaryHeap<T>,
    hash_set: HashSet<BlockHash>,
}

impl<T: BlockData + Ord> UniqueBlockData<T> {
    pub(crate) fn new() -> Self {
        UniqueBlockData {
            binary_heap: BinaryHeap::new(),
            hash_set: HashSet::new(),
        }
    }

    #[inline]
    pub(crate) fn push(&mut self, item: T) {
        if self.hash_set.insert(item.block_hash().clone()) {
            self.binary_heap.push(item)
        }
    }

    #[inline]
    pub(crate) fn pop(&mut self) -> Option<T> {
        let item = self.binary_heap.pop();
        if let Some(item) = item.as_ref() {
            assert!(
                self.hash_set.remove(item.block_hash()),
                "Relationship between the binary heap and teh hash set is corrupted"
            );
        }
        item
    }

    #[inline]
    pub(crate) fn peek(&self) -> Option<&T> {
        self.binary_heap.peek()
    }
}

impl<T> UniqueBlockData<T> {
    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.hash_set.len()
    }

    #[inline]
    pub(crate) fn is_empty(&self) -> bool {
        self.hash_set.is_empty()
    }
}

pub(crate) trait BlockData {
    fn block_hash(&self) -> &BlockHash;
}
