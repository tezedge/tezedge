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
    #[fail(display = "Failed to read from context error: {}", error)]
    ContextReadError {
        error: SkipListError
    },
    #[fail(display = "Failed to assign context_hash: {:?} to block_hash: {}, error: {}", context_hash, block_hash, error)]
    ContextHashAssignError {
        context_hash: String,
        block_hash: String,
        error: StorageError,
    },
    #[fail(display = "InvalidContextHash for context diff to commit, expected_context_hash: {:?}, context_hash: {:?}", expected_context_hash, context_hash)]
    InvalidContextHashError {
        expected_context_hash: Option<String>,
        context_hash: Option<String>,
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

/// Checks, if requested context_hash is the same as checkouted context_hash in context_diff
/// Import to ensure, that we are modifing correct diff for correct context_hash
#[macro_export]
macro_rules! ensure_eq_context_hash {
    ($x:expr, $y:expr) => {{
        let checkouted_diff_context_hash = &$y.predecessor_index.context_hash;
        if !($x.eq(checkouted_diff_context_hash)) {
            return Err(ContextError::InvalidContextHashError {
                expected_context_hash: $x.as_ref().map(|ch| HashType::ContextHash.bytes_to_string(&ch)),
                context_hash: checkouted_diff_context_hash.as_ref().map(|ch| HashType::ContextHash.bytes_to_string(&ch)),
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
    /// if parent_context_hash is empty, it means that its a commit_genesis a we dont assign context_hash to header
    fn commit(&mut self, block_hash: &BlockHash, parent_context_hash: &Option<ContextHash>, new_context_hash: &ContextHash, context_diff: &ContextDiff) -> Result<(), ContextError>;

    /// Checks context and resolves keys to be delete a place them to diff, and also deletes keys from diff
    fn delete_to_diff(&self, context_hash: &Option<ContextHash>, key_prefix_to_delete: &Vec<String>, context_diff: &mut ContextDiff) -> Result<(), ContextError>;

    /// Checks context and resolves keys to be delete a place them to diff, and also deletes keys from diff
    fn remove_recursively_to_diff(&self, context_hash: &Option<ContextHash>, key_prefix_to_remove: &Vec<String>, context_diff: &mut ContextDiff) -> Result<(), ContextError>;

    /// Checks context and copies subtree under 'from_key' to new subtree under 'to_key'
    fn copy_to_diff(&self, context_hash: &Option<ContextHash>, from_key: &Vec<String>, to_key: &Vec<String>, context_diff: &mut ContextDiff) -> Result<(), ContextError>;

    fn get_key(&self, context_index: &ContextIndex, key: &Vec<String>) -> Result<Option<Bucket<Vec<u8>>>, ContextError>;
}

fn to_key(key: &Vec<String>) -> String {
    key.join("/")
}

fn key_starts_with(key: &String, prefix: &Vec<String>) -> bool {
    key.starts_with(&to_key(prefix))
}

fn replace_key(key: &String, matched: &Vec<String>, replacer: &Vec<String>) -> String {
    key.replace(&to_key(matched), &to_key(replacer))
}

/// Struct points to context commmit hash or index, which is checkouted
pub struct ContextIndex {
    level: Option<usize>,
    pub context_hash: Option<ContextHash>,
}

impl ContextIndex {
    pub fn new(level: Option<usize>, context_hash: Option<ContextHash>) -> Self {
        ContextIndex { level, context_hash }
    }
}

/// Stuct which hold diff againts predecessor's context
pub struct ContextDiff {
    pub predecessor_index: ContextIndex,
    diff: ContextMap,
}

impl ContextDiff {
    pub fn new(predecessor_level: Option<usize>, predecessor_context_hash: Option<ContextHash>, diff: ContextMap) -> Self {
        ContextDiff {
            predecessor_index: ContextIndex::new(predecessor_level, predecessor_context_hash),
            diff,
        }
    }

    pub fn set(&mut self, context_hash: &Option<ContextHash>, key: &Vec<String>, value: &Vec<u8>) -> Result<(), ContextError> {
        ensure_eq_context_hash!(context_hash, &self);

        &self.diff.insert(to_key(key), Bucket::Exists(value.clone()));

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

    fn level_by_context_hash(&self, context_hash: &ContextHash) -> Result<usize, ContextError> {
        // find block header by context_hash, we need block level
        let block = self.block_storage
            .get_by_context_hash(context_hash)
            .map_err(|e| ContextError::ReadBlockError { context_hash: HashType::ContextHash.bytes_to_string(context_hash), error: e })?;
        if block.is_none() {
            return Err(ContextError::UnknownContextHashError { context_hash: HashType::ContextHash.bytes_to_string(context_hash) });
        }
        let block = block.unwrap();
        Ok(block.header.level() as usize)
    }

    pub fn get_by_key_prefix(&self, context_index: &ContextIndex, key: &Vec<String>) -> Result<Option<ContextMap>, ContextError> {
        if context_index.context_hash.is_none() && context_index.level.is_none() {
            return Ok(None);
        }

        // TODO: should be based just on context hash
        let level = if let Some(context_index_level) = context_index.level {
            context_index_level
        } else {
            self.level_by_context_hash(context_index.context_hash.as_ref().unwrap())?
        };

        let list = self.storage.read().expect("lock poisoning");
        list
            .get_prefix(level, &to_key(key))
            .map_err(|se| ContextError::ContextReadError { error: se })
    }
}

impl ContextApi for TezedgeContext {
    fn init_from_start(&self) -> ContextDiff {
        ContextDiff::new(None, None, Default::default())
    }

    fn checkout(&self, context_hash: &ContextHash) -> Result<ContextDiff, ContextError> {
        // TODO: should be based just on context hash
        let level = self.level_by_context_hash(&context_hash)?;

        Ok(
            ContextDiff::new(
                Some(level),
                Some(context_hash.clone()),
                Default::default(),
            )
        )
    }

    fn commit(&mut self, block_hash: &BlockHash, parent_context_hash: &Option<ContextHash>, new_context_hash: &ContextHash, context_diff: &ContextDiff) -> Result<(), ContextError> {
        ensure_eq_context_hash!(parent_context_hash, &context_diff);

        // add to context
        let mut writer = self.storage.write().expect("lock poisoning");
        // TODO: push to correct index by context_hash found by block_hash
        writer.push(&context_diff.diff)?;

        // associate block and context_hash
        if let Err(e) = self.block_storage.assign_to_context(block_hash, new_context_hash) {
            match e {
                StorageError::MissingKey => {
                    if parent_context_hash.is_some() {
                        return Err(
                            ContextError::ContextHashAssignError {
                                block_hash: HashType::BlockHash.bytes_to_string(block_hash),
                                context_hash: HashType::ContextHash.bytes_to_string(new_context_hash),
                                error: e,
                            }
                        );
                    } else {
                        // if parent_context_hash is empty, means it is commit_genesis, and block is not already stored, thats ok
                        ()
                    }
                }
                _ => return Err(
                    ContextError::ContextHashAssignError {
                        block_hash: HashType::BlockHash.bytes_to_string(block_hash),
                        context_hash: HashType::ContextHash.bytes_to_string(new_context_hash),
                        error: e,
                    }
                )
            };
        }

        Ok(())
    }

    fn delete_to_diff(&self, context_hash: &Option<ContextHash>, key_prefix_to_delete: &Vec<String>, context_diff: &mut ContextDiff) -> Result<(), ContextError> {
        ensure_eq_context_hash!(context_hash, &context_diff);
        // self.remove_recursively_to_diff(context_hash, key_prefix_to_delete, context_diff)
        let context_map_diff = &mut context_diff.diff;
        let context = self.get_by_key_prefix(&context_diff.predecessor_index, key_prefix_to_delete)?;
        if context.is_some() {
            let context = context.unwrap();
            for key in context.keys() {
                context_map_diff.insert(key.clone(), Bucket::Deleted);
            }
        }
        Ok(())
    }

    fn remove_recursively_to_diff(&self, context_hash: &Option<ContextHash>, key_prefix_to_remove: &Vec<String>, context_diff: &mut ContextDiff) -> Result<(), ContextError> {
        ensure_eq_context_hash!(context_hash, &context_diff);

        // at first remove keys from temp diff
        let context_map_diff = &mut context_diff.diff;
        context_map_diff.retain(|k, v| {
            if key_starts_with(k, key_prefix_to_remove) {
                match v {
                    Bucket::Deleted => true, // deleted stays in diff, because of previous delete from parent context, see bellow
                    _ => false
                }
            } else {
                // else keep in diff
                true
            }
        });

        // remove all keys with prefix from actual/parent context
        let context = self.get_by_key_prefix(&context_diff.predecessor_index, key_prefix_to_remove)?;
        if context.is_some() {
            let context = context.unwrap();
            for key in context.keys() {
                context_map_diff.insert(key.clone(), Bucket::Deleted);
            }
        }
        

        Ok(())
    }

    fn copy_to_diff(&self, context_hash: &Option<ContextHash>, from_key: &Vec<String>, to_key: &Vec<String>, context_diff: &mut ContextDiff) -> Result<(), ContextError> {
        ensure_eq_context_hash!(context_hash, &context_diff);

        // get keys from actual/parent context
        let final_context_to_copy = self.get_by_key_prefix(&context_diff.predecessor_index, from_key)?.unwrap_or(ContextMap::default());

        // merge the same keys from diff to final context
        for (key, bucket) in &final_context_to_copy {
            if key_starts_with(key, from_key) {
                match bucket {
                    Bucket::Exists(_) => {
                        let destination_key = replace_key(&key, from_key, to_key);
                        context_diff.diff.insert(destination_key, bucket.clone());
                        ()
                    }
                    _ => ()
                };
            }
        }

        Ok(())
    }

    fn get_key(&self, context_index: &ContextIndex, key: &Vec<String>) -> Result<Option<Bucket<Vec<u8>>>, ContextError> {
        if context_index.context_hash.is_none() && context_index.level.is_none() {
            return Ok(None);
        }

        // TODO: should be based just on context hash
        let level = if let Some(context_index_level) = context_index.level {
            context_index_level
        } else {
            self.level_by_context_hash(context_index.context_hash.as_ref().unwrap())?
        };

        let list = self.storage.read().expect("lock poisoning");
        list
            .get_key(level, &to_key(key))
            .map_err(|se| ContextError::ContextReadError { error: se })
    }
}