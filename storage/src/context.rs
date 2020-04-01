// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use failure::Fail;

use crypto::hash::{BlockHash, ContextHash, HashType};

use crate::{BlockStorage, BlockStorageReader, StorageError};
use crate::persistent::{ContextList, ContextMap};
use crate::skip_list::{Bucket, SkipListError};

/// Possible errors for context
#[derive(Debug, Fail)]
pub enum ContextError {
    #[fail(display = "Failed to save commit error: {}", error)]
    CommitWriteError {
        error: SkipListError
    },
    #[fail(display = "Failed to assign context_hash: {:?} to block_hash: {}, error: {}", context_hash, block_hash, error)]
    ContextHashAssignError {
        context_hash: String,
        block_hash: String,
        error: StorageError,
    },
    #[fail(display = "InvalidContextHash for context diff to commit, expected_parent_context_hash: {:?}, parent_context_hash: {:?}", expected_parent_context_hash, parent_context_hash)]
    InvalidContextHashError {
        expected_parent_context_hash: Option<String>,
        parent_context_hash: Option<String>,
    },
    #[fail(display = "Unknown context_hash: {:?}", context_hash)]
    UnknownContextHashError {
        context_hash: String,
    },
    #[fail(display = "Failed to read block for context_hash: {:?}, error: {}", context_hash, error)]
    ReadBlockError {
        context_hash: String,
        error: StorageError,
    },
}

impl From<SkipListError> for ContextError {
    fn from(error: SkipListError) -> Self {
        ContextError::CommitWriteError { error }
    }
}

#[macro_export]
macro_rules! ensure_eq_context_hash {
    ($x:expr, $y:expr) => {{
        if !($x.eq($y)) {
            return Err(ContextError::InvalidContextHashError {
                expected_parent_context_hash: $x.as_ref().map(|ch| HashType::ContextHash.bytes_to_string(&ch)),
                parent_context_hash: $y.as_ref().map(|ch| HashType::ContextHash.bytes_to_string(&ch)),
            });
        }
    }}
}

/// Abstraction on context manipulation
pub trait ContextApi {
    fn init_from_start(&self) -> ContextDiff;

    /// Checkout context for hash and return ContextDiff which is prepared for applying new successor block
    fn checkout(&self, context_hash: &ContextHash) -> Result<ContextDiff, ContextError>;

    /// Commit new generated context diff to storage
    fn commit(&mut self, block_hash: &BlockHash, parent_context_hash: &Option<ContextHash>, new_context_hash: &ContextHash, context_diff: &ContextDiff) -> Result<(), ContextError>;
}

/// Stuct which hold diff againts predecessor's context
pub struct ContextDiff {
    #[allow(dead_code)]
    predecessor_index: Option<usize>,
    predecessor_context_hash: Option<ContextHash>,
    diff: ContextMap,
}

impl ContextDiff {
    pub fn new(predecessor_index: Option<usize>, predecessor_context_hash: Option<ContextHash>, diff: ContextMap) -> Self {
        ContextDiff { predecessor_index, predecessor_context_hash, diff }
    }

    pub fn set(&mut self, context_hash: &Option<ContextHash>, key: &Vec<String>, value: &Vec<u8>) -> Result<(), ContextError> {
        ensure_eq_context_hash!(context_hash, &self.predecessor_context_hash);

        &self.diff.insert(key.join("/"), Bucket::Exists(value.clone()));

        Ok(())
    }
}

/// Actual context implementation with context skip list
pub struct TezedgeContext {
    block_storage: BlockStorage,
    storage: ContextList,
}

impl TezedgeContext {
    pub fn new(block_storage: BlockStorage, storage: ContextList) -> Self {
        TezedgeContext { block_storage, storage }
    }

    pub fn list(&self) -> &ContextList {
        &self.storage
    }
}

impl ContextApi for TezedgeContext {
    fn init_from_start(&self) -> ContextDiff {
        // TODO: initial context diff, should be initialized by genesis
        ContextDiff::new(None, None, Default::default())
    }

    fn checkout(&self, context_hash: &ContextHash) -> Result<ContextDiff, ContextError> {
        // find block header by context_hash, we need block level
        let block = self.block_storage
            .get_by_context_hash(context_hash)
            .map_err(|e| ContextError::ReadBlockError { context_hash: HashType::ContextHash.bytes_to_string(context_hash), error: e })?;
        if block.is_none() {
            return Err(ContextError::UnknownContextHashError { context_hash: HashType::ContextHash.bytes_to_string(context_hash) });
        }
        let block = block.unwrap();

        Ok(
            ContextDiff::new(
                Some(block.header.level() as usize),
                Some(context_hash.clone()),
                Default::default(),
            )
        )
    }

    fn commit(&mut self, block_hash: &BlockHash, parent_context_hash: &Option<ContextHash>, new_context_hash: &ContextHash, context_diff: &ContextDiff) -> Result<(), ContextError> {
        ensure_eq_context_hash!(parent_context_hash, &context_diff.predecessor_context_hash);

        // add to context
        let mut writer = self.storage.write().expect("lock poisoning");
        // TODO: push to correct index by context_hash found by block_hash
        writer.push(&context_diff.diff)?;

        // associate block and context_hash
        self.block_storage
            .assign_to_context(block_hash, new_context_hash)
            .map_err(|e| ContextError::ContextHashAssignError {
                block_hash: HashType::BlockHash.bytes_to_string(block_hash),
                context_hash: HashType::ContextHash.bytes_to_string(new_context_hash),
                error: e,
            })?;

        Ok(())
    }
}