// Copyright (c) SimpleStaking and Tezos-RS Contributors
// SPDX-License-Identifier: MIT

use std::cmp;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use log::trace;

use networking::p2p::encoding::prelude::*;
use storage::{BlockHeaderWithHash, IteratorMode, OperationsMetaStorage, OperationsMetaStorageDatabase, OperationsStorage, OperationsStorageDatabase, StorageError};
use tezos_encoding::hash::BlockHash;

pub struct OperationsState {
    operations_storage: OperationsStorage,
    operations_meta_storage: OperationsMetaStorage,
    missing_operations_for_blocks: Vec<BlockHash>,
}

impl OperationsState {

    pub fn new(db: Arc<OperationsStorageDatabase>, meta_db: Arc<OperationsMetaStorageDatabase>) -> Self {
        OperationsState {
            operations_storage: OperationsStorage::new(db),
            operations_meta_storage: OperationsMetaStorage::new(meta_db),
            missing_operations_for_blocks: Vec::new(),
        }
    }

    /// Process block header. This will create record in meta storage with
    /// unseen operations for the block header.
    ///
    /// If block header is not already present in storage, return `true`.
    ///
    /// If block is already present in storage return `false`.
    pub fn process_block_header(&mut self, block_header: &BlockHeaderWithHash) -> Result<bool, StorageError> {
        if !self.operations_meta_storage.contains(&block_header.hash)? {
            self.missing_operations_for_blocks.push(block_header.hash.clone());
            self.operations_meta_storage.put_block_header(block_header)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Process block operations. This will mark operations in store for the block as seen.
    ///
    /// If all block operations were processed return `true`.
    ///
    /// If there are still block operations to be processed return `false`.
    pub fn process_block_operations(&mut self, message: &OperationsForBlocksMessage) -> Result<bool, StorageError> {
        self.operations_storage.put_operations(message)?;
        self.operations_meta_storage.put_operations(message)?;
        self.operations_meta_storage.is_complete(&message.operations_for_block.hash)
    }

    pub fn drain_missing_operations(&mut self, n: usize) -> Result<Vec<MissingOperations>, StorageError> {
        let OperationsState { operations_meta_storage, missing_operations_for_blocks, .. } = self;
        let res = missing_operations_for_blocks
            .drain(0..cmp::min(missing_operations_for_blocks.len(), n))
            .map(|block_hash| MissingOperations {
                block_hash: block_hash.clone(),
                validation_passes: operations_meta_storage.get_missing_validation_passes(&block_hash).expect("Failed to get missing validation passes")
            })
            .collect();
        Ok(res)
    }

    pub fn push_missing_operations<Q: Iterator<Item=MissingOperations>>(&mut self, operations: Q) -> Result<(), StorageError>{
        for op in operations {
            if !self.operations_meta_storage.is_complete(&op.block_hash)? {
                self.missing_operations_for_blocks.push(op.block_hash);
            } else {
                trace!("Will not re-queue block {:?} because it has complete operations", &op.block_hash);
            }
        }
        Ok(())
    }

    pub fn has_missing_operations(&self) -> bool {
        !self.missing_operations_for_blocks.is_empty()
    }

    pub fn hydrate(&mut self) -> Result<(), StorageError> {
        let OperationsState { operations_meta_storage, missing_operations_for_blocks, .. } = self;
        for (key, value) in operations_meta_storage.iter(IteratorMode::Start)? {
            if !value?.is_complete() {
                missing_operations_for_blocks.push(key?);
            }
        }

        Ok(())
    }

}

#[derive(Clone, Debug)]
pub struct MissingOperations {
    pub block_hash: BlockHash,
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
                    hash: ops.block_hash.clone(),
                    validation_pass: *vp
                }
            })
            .collect()
    }
}