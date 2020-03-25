// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;

use crate::skip_list::{content::ListValue, Bucket, SkipList, TypedSkipList, SkipListError};
use crate::persistent::{Codec};

pub type ContextMap = HashMap<String, Bucket<Vec<u8>>>;
const SAVE_ON_BLOCK_LEVEL: usize = 2048;

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

// use a hybrid approach between a skiplist and a flatlist
// store all the changes, and every 2048 level, store the whole context
// 0, 1 ..   
//
impl<K: Codec, V: Codec, C: ListValue<K, V> + Clone> TypedSkipList<K, V, C> for ContextRamStorage<C> {
    fn get(&self, index: usize) -> Result<Option<C>, SkipListError> {
        if self.contains(index) {
            // return Ok(None);
            // Ok(Some(self.context_ram_map.get(&index).unwrap().clone()))
            if index != 0 && index % SAVE_ON_BLOCK_LEVEL == 0 {
                println!("Selected checkpoint level: {} -> serving whole context", index);
                Ok(Some(self.context_ram_map.get(&index).unwrap().clone()))
            } else {
                let stored = index / SAVE_ON_BLOCK_LEVEL;
                let start_index = stored * SAVE_ON_BLOCK_LEVEL;

                // println!("SELECTED LEVEL: {}     CHECKPOINT: {}", index, start_index);

                let mut merged_context: C = Default::default();
                for ctxt_index in start_index..index + 1 {
                    let current_context = self.context_ram_map.get(&ctxt_index).unwrap().clone();

                    merged_context.merge(&current_context);
                }
                Ok(Some(merged_context))
            }
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

        // make a checkpoint with the merged changes up to this level
        if index != 0 && index % SAVE_ON_BLOCK_LEVEL == 0 {
            println!("Level: {} -> saving whole context", index);
            let mut merged_context: C = Default::default();
            let mut current_context;

            // merge all the changes since the last checkpoint
            for level in index - SAVE_ON_BLOCK_LEVEL..index {
                if let Some(ctxt) = self.context_ram_map.get_mut(&level) {
                    current_context = ctxt.clone();
                } else {
                    current_context = Default::default();
                }
                merged_context.merge(&current_context);
            }
            // lastly, merge the current changes
            merged_context.merge(&value);
            
            // insert into context map
            self.context_ram_map.insert(index, merged_context);
        } else {
            // println!("Level: {} -> saving context difference", index);

            // insert the change into the context map
            self.context_ram_map.insert(index, value.clone());
        }

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