// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;

use crate::skip_list::{content::ListValue, Bucket, SkipList, TypedSkipList, SkipListError};
use crate::persistent::{Codec};

pub type ContextMap = HashMap<String, Bucket<Vec<u8>>>;

// Note: this flag is temporary, this behaviour should be triggered by the rolling node mode;
const ENABLE_DROP_FLAG: bool = true;

// Note: This should be dependant on the protocol constants
const BLOCKS_PER_CYCLE: usize = 2048;

// Note: Also dependant on protocol constants
// Number of cycles into the past to keep 
const PRESERVED_CYCLES: usize = 5;

const PRESERVED_BLOCKS: usize = BLOCKS_PER_CYCLE * PRESERVED_CYCLES;

#[derive(Clone)]
pub struct ContextRamStorage<C: Clone> {
    context_ram_map: HashMap<usize, C>,
    last_index: usize,
    checkpoint: usize,
}

impl<C: Clone> SkipList for ContextRamStorage<C> {
    fn len(&self) -> usize {
        self.last_index
    }

    #[inline]
    fn levels(&self) -> usize {
        1
    }

    #[inline]
    fn contains(&self, index: usize) -> bool {
        self.last_index > index && self.checkpoint <= index
    }
}

// use a hybrid approach between a skiplist and a flatlist
// store all the changes, and every BLOCKS_PER_CYCLE level, store the whole context 
impl<K: Codec, V: Codec, C: ListValue<K, V> + Clone> TypedSkipList<K, V, C> for ContextRamStorage<C> {
    fn get(&self, index: usize) -> Result<Option<C>, SkipListError> {
        if self.contains(index) {
            if index != 0 && index % BLOCKS_PER_CYCLE == 0 {
                Ok(Some(self.context_ram_map.get(&index).unwrap().clone()))
            } else {
                let stored = index / BLOCKS_PER_CYCLE;
                let start_index = stored * BLOCKS_PER_CYCLE;

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
        let index = self.len();

        // make a checkpoint with the merged changes up to this level
        if index != 0 && index % BLOCKS_PER_CYCLE == 0 {
            let mut merged_context: C = Default::default();
            let mut current_context;

            // merge all the changes since the last checkpoint
            for level in index - BLOCKS_PER_CYCLE..index {
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
            // insert the change into the context map
            self.context_ram_map.insert(index, value.clone());
        }

        // this is experimental garbage collection, for future rolling mode
        // we drop the ctxt when the drop flag is enabled every PRESERVED_BLOCKS,
        // special cases include the first PRESERVED_BLOCKS, when the chain has no past ctxt (checkpoint on level 0) so there is nothing to drop
        // hence the clause: index != PRESERVED_BLOCKS
        if ENABLE_DROP_FLAG && index % PRESERVED_BLOCKS == 0 && index != 0 && index != PRESERVED_BLOCKS {
            println!("Cleaning ctxt data - index: {}", index);
            self.drop(index - PRESERVED_BLOCKS)?;
        }

        // increment the last_index by one
        self.last_index += 1;
        Ok(())
    }

    fn diff(&self, _from: usize, _to: usize) -> Result<Option<C>, SkipListError> {
        unimplemented!()
    }

    fn drop(&mut self, new_checkpoint: usize) -> Result<(), SkipListError> {
        println!("Checkpoint reached: old checkpoint: {} -> new checkpoint: {}", self.checkpoint, new_checkpoint);
        self.checkpoint = new_checkpoint;
        
        // only keep levels above the new checkpoint
        Ok(self.context_ram_map.retain(|&k, _| k >= new_checkpoint))
    }
}

impl<C: Default + Clone> ContextRamStorage<C> {
    pub fn new() -> Self {
        let context_ram_map = HashMap::new();
        let last_index = 0;
        let checkpoint = 0;

        Self{context_ram_map, last_index, checkpoint}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn test_drop() {
        // run only, if ENABLE_DROP_FLAG is set
        if ENABLE_DROP_FLAG {
            let mut map = ContextRamStorage::<ContextMap>::new();

            // bootstrap to the level, where the first drop occures
            // the first checkpoint should be at PRESERVED_BLOCKS
            const BOOTSTRAP_INDEX: usize = PRESERVED_BLOCKS * 2; 

            // botstrap simulation to block 8192
            for _ in 0..BOOTSTRAP_INDEX + 1 {
                let mut dummy: ContextMap = HashMap::new();
                dummy.insert("key".to_string(), Bucket::Exists(vec![0,0]));
                map.push(dummy.clone()).unwrap();
            }

            // check indexes bellow checkpoint level, everything bellow the new checkpoint should be dropped
            let ctxt = map.get(0).unwrap();
            assert!(ctxt.is_none());
            let ctxt = map.get(PRESERVED_BLOCKS - 1).unwrap();
            assert!(ctxt.is_none());
            let ctxt = map.get(PRESERVED_BLOCKS / 2).unwrap();
            assert!(ctxt.is_none());
            
            // naturally ctxt should exist on BOOTSTRAP_INDEX
            let ctxt = map.get(BOOTSTRAP_INDEX).unwrap();
            assert!(ctxt.is_some());

            // check the checkpoint index, it should exist
            let ctxt = map.get(PRESERVED_BLOCKS).unwrap();
            assert!(ctxt.is_some());
        }
    }
}