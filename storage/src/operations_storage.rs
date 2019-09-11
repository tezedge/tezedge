use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use networking::p2p::encoding::prelude::*;
use tezos_encoding::hash::HashRef;

use crate::BlockHeaderWithHash;

pub struct Operations {
    pub(crate) block_hash: HashRef,
    values: Vec<Option<OperationsForBlocksMessage>>
}

impl PartialEq for Operations {
    fn eq(&self, other: &Self) -> bool {
        self.block_hash == other.block_hash
    }
}

impl Eq for Operations {}

impl Hash for Operations {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.block_hash.hash(state);
    }
}

impl Operations {

    pub fn insert(&mut self, value: OperationsForBlocksMessage) {
        let op_idx = value.operations_for_block.validation_pass as usize;
        self.values[op_idx] = Some(value);
    }

    pub fn is_complete(&self) -> bool {
        self.values.iter().all(Option::is_some)
    }

    pub fn get_missing_validation_passes(&self) -> Vec<i8> {
        self.values.iter().enumerate()
            .filter(|(_, value)| value.is_none())
            .map(|(idx, _)| idx as i8)
            .collect()
    }
}

/// Structure for representing in-memory db for - just for demo purposes.
pub struct OperationsStorage(HashMap<HashRef, Operations>);

impl OperationsStorage {

    pub fn new() -> Self {
        OperationsStorage(HashMap::new())
    }

    pub fn contains_block_hash(&self, block_hash: &HashRef) -> bool {
        self.0.contains_key(block_hash)
    }

    pub fn initialize_operations(&mut self, block_header: &BlockHeaderWithHash) {
        let operations = Operations {
            block_hash: block_header.hash.clone(),
            values: vec![None; block_header.header.validation_pass as usize],
        };
        self.0.insert(block_header.hash.clone(), operations);
    }

    pub fn get_operations_mut(&mut self, block_hash: &HashRef) -> Option<&mut Operations> {
        self.0.get_mut(block_hash)
    }

    pub fn get_operations(&mut self, block_hash: &HashRef) -> Option<&Operations> {
        self.0.get(block_hash)
    }

    pub fn is_present(&self, block_hash: &HashRef) -> bool {
        self.0.contains_key(block_hash)
    }
}

