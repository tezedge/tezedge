// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;

use crate::skip_list::{content::ListValue, Bucket, SkipList, TypedSkipList, SkipListError};
use crate::persistent::{Codec};

pub type ContextMap = HashMap<String, Bucket<Vec<u8>>>;

#[derive(Clone)]
pub struct ContextRamStorage<C: Clone> {
    context_ram_map: HashMap<usize, C>,
}

impl<C: Clone> SkipList for ContextRamStorage<C> {
    fn len(&self) -> usize {
        self.context_ram_map.len()
    }

    #[inline]
    fn levels(&self) -> usize {
        1
    }

    #[inline]
    fn contains(&self, index: usize) -> bool {
        self.context_ram_map.len() > index
    }
}

impl<K: Codec, V: Codec, C: ListValue<K, V> + Clone> TypedSkipList<K, V, C> for ContextRamStorage<C> {
    fn get(&self, index: usize) -> Result<Option<C>, SkipListError> {
        if self.contains(index) {
            // return Ok(None);
            Ok(Some(self.context_ram_map.get(&index).unwrap().clone()))
        } else {
            Ok(None)
        }
    }

    fn get_key(&self, index: usize, key: &K) -> Result<Option<V>, SkipListError> {
        Ok(self.get(index)?.and_then(|c| c.get(key)))
    }

    fn get_raw(&self, lane_level: usize, index: usize) -> Result<Option<C>, SkipListError> {
        if lane_level == 0 {
            self.get(index)
        } else {
            Ok(None)
        }
    }

    fn push(&mut self, value: C) -> Result<(), SkipListError> {
        let index = self.context_ram_map.len();
        let mut current_context;
        if let Some(ctxt) = self.context_ram_map.get_mut(&(index - 1)) {
            current_context = ctxt.clone();
        } else {
            current_context = Default::default();
        }
        current_context.merge(&value);
        self.context_ram_map.insert(index, current_context);

        Ok(())
    }

    fn diff(&self, _from: usize, _to: usize) -> Result<Option<C>, SkipListError> {
        unimplemented!()
    }
}

impl<C: Default + Clone> ContextRamStorage<C> {
    pub fn new() -> Self {
        let context_ram_map = HashMap::new();

        Self{context_ram_map}
    }
}