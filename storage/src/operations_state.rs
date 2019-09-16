use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use log::debug;

use networking::p2p::encoding::prelude::*;
use tezos_encoding::hash::HashRef;

use crate::{BlockHeaderWithHash, StorageError};
use crate::operations_storage::{OperationsMetaStorage, OperationsMetaStorageDatabase, OperationsStorage, OperationsStorageDatabase};

pub struct OperationsState {
    operations_storage: OperationsStorage,
    meta_storage: OperationsMetaStorage,
    missing_operations_for_blocks: HashSet<HashRef>,
}

impl OperationsState {

    pub fn new(db: Arc<OperationsStorageDatabase>, meta_db: Arc<OperationsMetaStorageDatabase>) -> Self {
        OperationsState {
            operations_storage: OperationsStorage::new(db),
            meta_storage: OperationsMetaStorage::new(meta_db),
            missing_operations_for_blocks: HashSet::new(),
        }
    }

    pub fn insert_block_header(&mut self, block_header: &BlockHeaderWithHash) -> Result<(), StorageError> {
        if !self.meta_storage.contains(&block_header.hash)? {
            self.missing_operations_for_blocks.insert(block_header.hash.clone());
            self.meta_storage.initialize(block_header)?;
        }
        Ok(())
    }

    pub fn insert_operations(&mut self, message: &OperationsForBlocksMessage) -> Result<(), StorageError> {
        let hash_ref = HashRef::new(message.operations_for_block.hash.clone());
        self.operations_storage.insert(message)?;

        self.meta_storage.insert(message)?;
        if self.meta_storage.is_complete(&hash_ref)? {
            debug!("Block {:?} has complete operations", &hash_ref);
            self.missing_operations_for_blocks.remove(&hash_ref);
        }
        Ok(())
    }

    pub fn move_to_queue(&mut self, n: usize) -> Result<Vec<MissingOperations>, StorageError> {
        let OperationsState { meta_storage, missing_operations_for_blocks, .. } = self;
        let res = missing_operations_for_blocks
            .drain()
            .take(n)
            .map(|block_hash| MissingOperations {
                block_hash: block_hash.clone(),
                validation_passes: meta_storage.get_missing_validation_passes(&block_hash).expect("Failed to get missing validation passes")
            })
            .collect();
        Ok(res)
    }

    pub fn return_from_queue<Q: Iterator<Item=MissingOperations>>(&mut self, operations: Q) -> Result<(), StorageError>{
        for op in operations {
            if !self.meta_storage.is_complete(&op.block_hash)? {
                self.missing_operations_for_blocks.insert(op.block_hash);
            } else {
                debug!("Will not re-queue block {:?} because it has complete operations", &op.block_hash);
            }
        }
        Ok(())
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