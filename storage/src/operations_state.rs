use std::collections::HashSet;
use std::hash::{Hash, Hasher};

use networking::p2p::encoding::prelude::*;
use tezos_encoding::hash::{HashRef, ToHashRef};

use crate::BlockHeaderWithHash;
use crate::operations_storage::OperationsStorage;

pub struct OperationsState {
    storage: OperationsStorage,
    missing_operations_for_blocks: HashSet<HashRef>,
}

impl OperationsState {

    pub fn new() -> Self {
        OperationsState {
            storage: OperationsStorage::new(),
            missing_operations_for_blocks: HashSet::new(),
        }
    }

    pub fn insert_block_header(&mut self, block_header: &BlockHeaderWithHash) {
        if !self.storage.is_present(&block_header.hash) {
            self.storage.initialize_operations(block_header);
            self.missing_operations_for_blocks.insert(block_header.hash.clone());
        }
    }

    pub fn insert_operations(&mut self, message: OperationsForBlocksMessage) {
        let block_hash = (&message.operations_for_block.hash).to_hash_ref();
        if let Some(operations) = self.storage.get_operations_mut(&block_hash) {
            operations.insert(message);
            if operations.is_complete() {
                self.missing_operations_for_blocks.remove(&operations.block_hash);
            }
        }
    }

    pub fn schedule_block_hash(&mut self, block_hash: HashRef) {
        if !self.storage.is_present(&block_hash) {
            self.missing_operations_for_blocks.insert(block_hash);
        }
    }

    pub fn move_to_queue(&mut self, n: usize) -> Vec<MissingOperations> {
        let OperationsState { storage, missing_operations_for_blocks } = self;
        missing_operations_for_blocks
            .drain()
            .take(n)
            .filter_map(|block_hash| match storage.get_operations(&block_hash) {
                Some(operations) => Some(MissingOperations {
                    block_hash: operations.block_hash.clone(),
                    validation_passes: operations.get_missing_validation_passes()
                }),
                None => None
            })
            .collect()
    }

    pub fn return_from_queue<Q: Iterator<Item=MissingOperations>>(&mut self, operations: Q) {
        operations
            .for_each(|op| {
                self.missing_operations_for_blocks.insert(op.block_hash);
            })
    }

    pub fn has_missing_operations(&self) -> bool {
        !self.missing_operations_for_blocks.is_empty()
    }
}

#[derive(Clone)]
pub struct MissingOperations {
    pub block_hash: HashRef,
    pub validation_passes: HashSet<i8>
}

impl PartialEq for MissingOperations {
    fn eq(&self, other: &Self) -> bool {
        self.block_hash == other.block_hash
    }
}

impl Eq for MissingOperations {}

impl Hash for MissingOperations {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.block_hash.hash(state);
    }
}


impl From<&MissingOperations> for Vec<OperationsForBlock> {
    fn from(ops: &MissingOperations) -> Self {
        ops.validation_passes
            .iter()
            .map(|vp| {
                OperationsForBlock {
                    hash: ops.block_hash.get_hash(),
                    validation_pass: *vp
                }
            })
            .collect()
    }
}