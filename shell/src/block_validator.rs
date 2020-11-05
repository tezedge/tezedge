// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use storage::{BlockMetaStorage, BlockMetaStorageReader, BlockStorage, BlockStorageReader};
use storage::persistent::PersistentStorage;

/// We need to access block validation process from three different places/cases:
///
/// 1. batch of blocks apply for bootstrap process
/// 2. single block apply for CurrentHead processing
/// 3. single block apply for inject/block RPC
///
/// So this is the place with common applying logic
///
/// TODO: only this should manage and work with writable protocol runner context
///
/// block_validator - push and wait
pub struct BlockValidator {
    /// Block storage
    block_storage: Box<dyn BlockStorageReader>,
    block_meta_storage: Box<dyn BlockMetaStorageReader>,
}

impl BlockValidator {
    pub fn new(persistent_storage: &PersistentStorage) -> Self {
        BlockValidator {
            block_storage: Box::new(BlockStorage::new(persistent_storage)),
            block_meta_storage: Box::new(BlockMetaStorage::new(persistent_storage)),
        }
    }

    pub fn apply_block(&self) {

    }

    pub fn apply_blocks(&self) {

    }
}