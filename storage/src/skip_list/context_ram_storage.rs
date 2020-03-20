// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use failure::{bail, format_err};
use std::sync::Arc;
use std::collections::HashMap;
use getset::Getters;

use crate::skip_list::{content::ListValue, Bucket, SkipList, TypedSkipList, SkipListError};
use crate::persistent::Codec;

pub type ContextMap = HashMap<String, Bucket<Vec<u8>>>;

#[derive(Getters)]
pub struct ContextRamStorage<C> {
    context_ram_map: HashMap<String, ContextMap>,
    context_blocks: Vec<String>,
    current_context: C,
}

impl<C> SkipList for ContextRamStorage<C> {
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

impl<K: Codec, V: Codec, C: ListValue<K, V>> TypedSkipList<K, V, C> for ContextRamStorage<C> {
    fn get(&self, index: usize) -> Result<Option<C>, SkipListError> {
        if self.contains(index) {
            // return Ok(None);
            Ok(self.context_ram_map.get(index))
        } else {
            Ok(None)
        }
        // take a slice from the begining to the desired block
        // let relevant_blocks = &self.context_blocks[0..index + 1];

        // // merge the context changes starting from the requested block to genesis
        // let mut context: HashMap<String, Bucket<Vec<u8>>> = HashMap::<String, Bucket<Vec<u8>>>::new();
        // for block_hash in relevant_blocks {
        //     let block_context: HashMap<String, Bucket<Vec<u8>>> = self.context_ram_map.get(block_hash).unwrap().clone();
        //     context.extend(block_context);
        // }
        // Ok(Some(context))
    }
}

impl<C: Default> ContextRamStorage<C> {
    pub fn new() -> Self {
        let context_ram_map = HashMap::new();
        let context_blocks = Vec::<String>::new();

        Self{context_ram_map, context_blocks, current_context: Default::default()}
    }

    // pub fn push_state(&mut self, block_hash: &str, state: ContextMap) -> Result<(), failure::Error> {
    //     self.context_ram_map.insert(block_hash.to_string(), state);
    //     Ok(())
    // }

    // pub fn push_block_hash(&mut self, block_hash: &str) -> Result<(), failure::Error> {
    //     self.context_blocks.push(block_hash.to_string());
    //     Ok(())
    // }

    // pub fn get_key(&self, level: usize, key: &str) -> Result<Option<Bucket<Vec<u8>>>, failure::Error> {
        
    //     let context;
    //     if let Some(ctxt) = self.get(level)? {
    //         context = ctxt;
    //     } else {
    //         return Ok(None);
    //     }

    //     let value;
    //     // get from hashmap
    //     if let Some(data) = context.get(key) {
    //         value = data;
    //     } else {
    //         return Ok(None);
    //     }

    //     Ok(Some(value.clone()))
    // }

    // pub fn get(&self, level: usize) -> Result<Option<ContextMap>, failure::Error> {
    //     // look for the block index (order of insertion)
    //     //let block_position = self.context_blocks.iter().position(|x| x == block_id).unwrap();

        
    // }
}