use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use networking::p2p::encoding::prelude::*;
use tezos_encoding::hash::{HashRef, ToHashRef};

use crate::{BlockHeaderWithHash, StorageError};
use crate::operations_storage::{OperationsStorage, OperationsStorageDatabase};

pub struct OperationsState {
    storage: OperationsStorage,
    missing_operations_for_blocks: HashSet<HashRef>,
}

impl OperationsState {

    pub fn new(db: Arc<OperationsStorageDatabase>) -> Self {
        OperationsState {
            storage: OperationsStorage::new(db),
            missing_operations_for_blocks: HashSet::new(),
        }
    }

    pub fn insert_block_header(&mut self, block_header: &BlockHeaderWithHash) -> Result<(), StorageError> {
        if !self.storage.contains_operations(&block_header.hash)? {
            self.missing_operations_for_blocks.insert(block_header.hash.clone());
            self.storage.initialize(block_header)?;
        }
        Ok(())
    }

    pub fn insert_operations(&mut self, message: OperationsForBlocksMessage) -> Result<(), StorageError> {
        let hash_ref = HashRef::new(message.operations_for_block.hash.clone());
        self.storage.insert(message)?;
        if self.storage.is_complete(&hash_ref) {
            self.missing_operations_for_blocks.remove(&hash_ref);
        }
        Ok(())
    }

    pub fn schedule_block_hash(&mut self, block_hash: HashRef) {
        if !self.storage.contains_operations(&block_hash) {
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