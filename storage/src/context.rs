// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::array::TryFromSliceError;
use std::convert::TryFrom;
use std::convert::TryInto;
use std::num::TryFromIntError;
use std::sync::{Arc, RwLock};

use failure::Fail;

use crypto::hash::{BlockHash, ContextHash, FromBytesError};

use crate::merkle_storage::{
    ContextKey, ContextValue, EntryHash, MerkleError, MerkleStorage, StringTreeEntry,
};
use crate::merkle_storage_stats::MerkleStoragePerfReport;
use crate::{BlockStorage, BlockStorageReader, StorageError};

// An unique tree identifier during a block application
pub type TreeId = i32;

/// Abstraction on context manipulation
pub trait ContextApi {
    // set key-value
    fn set(
        &mut self,
        context_hash: &Option<ContextHash>,
        new_tree_id: TreeId,
        key: &ContextKey,
        value: &ContextValue,
    ) -> Result<(), ContextError>;
    // checkout context for hash
    fn checkout(&self, context_hash: &ContextHash) -> Result<(), ContextError>;
    // commit current context diff to storage
    // if parent_context_hash is empty, it means that it's a commit_genesis and we don't assign context_hash to header
    fn commit(
        &mut self,
        block_hash: &BlockHash,
        parent_context_hash: &Option<ContextHash>,
        author: String,
        message: String,
        date: i64,
    ) -> Result<ContextHash, ContextError>;
    fn delete_to_diff(
        &self,
        context_hash: &Option<ContextHash>,
        new_tree_id: TreeId,
        key_prefix_to_delete: &ContextKey,
    ) -> Result<(), ContextError>;
    fn remove_recursively_to_diff(
        &self,
        context_hash: &Option<ContextHash>,
        new_tree_id: TreeId,
        key_prefix_to_remove: &ContextKey,
    ) -> Result<(), ContextError>;
    // copies subtree under 'from_key' to new subtree under 'to_key'
    fn copy_to_diff(
        &self,
        context_hash: &Option<ContextHash>,
        new_tree_id: TreeId,
        from_key: &ContextKey,
        to_key: &ContextKey,
    ) -> Result<(), ContextError>;
    // get value for key
    fn get_key(&self, key: &ContextKey) -> Result<ContextValue, ContextError>;
    // mem - check if value exists
    fn mem(&self, key: &ContextKey) -> Result<bool, ContextError>;
    // dirmem - check if directory exists
    fn dirmem(&self, key: &ContextKey) -> Result<bool, ContextError>;
    // get value for key from a point in history indicated by context hash
    fn get_key_from_history(
        &self,
        context_hash: &ContextHash,
        key: &ContextKey,
    ) -> Result<Option<ContextValue>, ContextError>;
    // get a list of all key-values under a certain key prefix
    fn get_key_values_by_prefix(
        &self,
        context_hash: &ContextHash,
        prefix: &ContextKey,
    ) -> Result<Option<Vec<(ContextKey, ContextValue)>>, MerkleError>;
    // get entire context tree in string form for JSON RPC
    fn get_context_tree_by_prefix(
        &self,
        context_hash: &ContextHash,
        prefix: &ContextKey,
        depth: Option<usize>,
    ) -> Result<StringTreeEntry, MerkleError>;

    // get currently checked out hash
    fn get_last_commit_hash(&self) -> Option<Vec<u8>>;
    // get stats from merkle storage
    fn get_merkle_stats(&self) -> Result<MerkleStoragePerfReport, ContextError>;

    /// TODO: TE-203 - remove when context_listener will not be used
    // check if context_hash is committed
    fn is_committed(&self, context_hash: &ContextHash) -> Result<bool, ContextError>;

    fn set_merkle_root(&mut self, tree_id: TreeId) -> Result<(), MerkleError>;

    fn get_merkle_root(&mut self) -> EntryHash;

    fn block_applied(&self) -> Result<(), ContextError>;

    fn cycle_started(&self) -> Result<(), ContextError>;

    fn get_memory_usage(&self) -> Result<usize, ContextError>;
}

impl ContextApi for TezedgeContext {
    fn set(
        &mut self,
        _context_hash: &Option<ContextHash>,
        new_tree_id: TreeId,
        key: &ContextKey,
        value: &ContextValue,
    ) -> Result<(), ContextError> {
        let mut merkle = self.merkle.write().expect("lock poisoning");
        merkle.set(new_tree_id, key, value)?;

        Ok(())
    }

    fn checkout(&self, context_hash: &ContextHash) -> Result<(), ContextError> {
        let context_hash_arr: EntryHash = context_hash.as_ref().as_slice().try_into()?;
        let mut merkle = self.merkle.write().expect("lock poisoning");
        merkle.checkout(&context_hash_arr)?;

        Ok(())
    }

    fn commit(
        &mut self,
        block_hash: &BlockHash,
        parent_context_hash: &Option<ContextHash>,
        author: String,
        message: String,
        date: i64,
    ) -> Result<ContextHash, ContextError> {
        let mut merkle = self.merkle.write().expect("lock poisoning");

        let date: u64 = date.try_into()?;
        let commit_hash = merkle.commit(date, author, message)?;
        let commit_hash = ContextHash::try_from(&commit_hash[..])?;

        // associate block and context_hash
        if let Err(e) = self
            .block_storage
            .assign_to_context(block_hash, &commit_hash)
        {
            match e {
                StorageError::MissingKey => {
                    // TODO: is this needed? check it when removing assign_to_context
                    if parent_context_hash.is_some() {
                        return Err(ContextError::ContextHashAssignError {
                            block_hash: block_hash.to_base58_check(),
                            context_hash: commit_hash.to_base58_check(),
                            error: e,
                        });
                    } else {
                        // TODO: do correctly assignement on one place, or remove this assignemnt - it is not needed
                        // if parent_context_hash is empty, means it is commit_genesis, and block is not already stored, thats ok
                        // but we need to storage assignment elsewhere
                    }
                }
                _ => {
                    return Err(ContextError::ContextHashAssignError {
                        block_hash: block_hash.to_base58_check(),
                        context_hash: commit_hash.to_base58_check(),
                        error: e,
                    })
                }
            };
        }

        Ok(commit_hash)
    }

    fn delete_to_diff(
        &self,
        _context_hash: &Option<ContextHash>,
        new_tree_id: TreeId,
        key_prefix_to_delete: &ContextKey,
    ) -> Result<(), ContextError> {
        let mut merkle = self.merkle.write().expect("lock poisoning");
        merkle.delete(new_tree_id, key_prefix_to_delete)?;
        Ok(())
    }

    fn remove_recursively_to_diff(
        &self,
        _context_hash: &Option<ContextHash>,
        new_tree_id: TreeId,
        key_prefix_to_remove: &ContextKey,
    ) -> Result<(), ContextError> {
        let mut merkle = self.merkle.write().expect("lock poisoning");
        merkle.delete(new_tree_id, key_prefix_to_remove)?;
        Ok(())
    }

    fn copy_to_diff(
        &self,
        _context_hash: &Option<ContextHash>,
        new_tree_id: TreeId,
        from_key: &ContextKey,
        to_key: &ContextKey,
    ) -> Result<(), ContextError> {
        let mut merkle = self.merkle.write().expect("lock poisoning");
        merkle.copy(new_tree_id, from_key, to_key)?;
        Ok(())
    }

    fn get_key(&self, key: &ContextKey) -> Result<ContextValue, ContextError> {
        let mut merkle = self.merkle.write().expect("lock poisoning");
        let val = merkle.get(key)?;
        Ok(val)
    }

    fn mem(&self, key: &ContextKey) -> Result<bool, ContextError> {
        let mut merkle = self.merkle.write().expect("lock poisoning");
        let val = merkle.mem(key)?;
        Ok(val)
    }

    fn dirmem(&self, key: &ContextKey) -> Result<bool, ContextError> {
        let mut merkle = self.merkle.write().expect("lock poisoning");
        let val = merkle.dirmem(key)?;
        Ok(val)
    }

    fn get_key_from_history(
        &self,
        context_hash: &ContextHash,
        key: &ContextKey,
    ) -> Result<Option<ContextValue>, ContextError> {
        let context_hash_arr: EntryHash = context_hash.as_ref().as_slice().try_into()?;
        let mut merkle = self.merkle.write().expect("lock poisoning");
        match merkle.get_history(&context_hash_arr, key) {
            Err(MerkleError::ValueNotFound { key: _ }) => Ok(None),
            Err(MerkleError::EntryNotFound { hash: _ }) => {
                Err(ContextError::UnknownContextHashError {
                    context_hash: context_hash.to_base58_check(),
                })
            }
            Err(err) => Err(ContextError::MerkleStorageError { error: err }),
            Ok(val) => Ok(Some(val)),
        }
    }

    fn get_key_values_by_prefix(
        &self,
        context_hash: &ContextHash,
        prefix: &ContextKey,
    ) -> Result<Option<Vec<(ContextKey, ContextValue)>>, MerkleError> {
        let context_hash_arr: EntryHash = context_hash.as_ref().as_slice().try_into()?;
        let mut merkle = self.merkle.write().expect("lock poisoning");
        merkle.get_key_values_by_prefix(&context_hash_arr, prefix)
    }

    fn get_context_tree_by_prefix(
        &self,
        context_hash: &ContextHash,
        prefix: &ContextKey,
        depth: Option<usize>,
    ) -> Result<StringTreeEntry, MerkleError> {
        let context_hash_arr: EntryHash = context_hash.as_ref().as_slice().try_into()?;
        let mut merkle = self.merkle.write().expect("lock poisoning");
        merkle.get_context_tree_by_prefix(&context_hash_arr, prefix, depth)
    }

    fn get_last_commit_hash(&self) -> Option<Vec<u8>> {
        let merkle = self.merkle.read().expect("lock poisoning");
        merkle.get_last_commit_hash().map(|x| x.to_vec())
    }

    fn get_merkle_stats(&self) -> Result<MerkleStoragePerfReport, ContextError> {
        let merkle = self.merkle.read().expect("lock poisoning");
        Ok(merkle.get_merkle_stats()?)
    }

    fn is_committed(&self, context_hash: &ContextHash) -> Result<bool, ContextError> {
        self.block_storage
            .contains_context_hash(context_hash)
            .map_err(|e| ContextError::StorageError { error: e })
    }

    fn set_merkle_root(&mut self, tree_id: TreeId) -> Result<(), MerkleError> {
        let mut merkle = self.merkle.write().expect("lock poisoning");
        merkle.stage_checkout(tree_id)
    }

    fn get_merkle_root(&mut self) -> EntryHash {
        let merkle = self.merkle.read().expect("lock poisoning");
        merkle.get_staged_root_hash()
    }

    fn block_applied(&self) -> Result<(), ContextError> {
        let mut merkle = self.merkle.write().expect("lock poisoning");
        Ok(merkle.mark_entries_from_last_commit_as_used()?)
    }

    fn cycle_started(&self) -> Result<(), ContextError> {
        let mut merkle = self.merkle.write().expect("lock poisoning");
        Ok(merkle.start_new_cycle()?)
    }

    fn get_memory_usage(&self) -> Result<usize, ContextError> {
        let merkle = self.merkle.write().expect("lock poisoning");
        Ok(merkle.get_memory_usage()?)
    }
}

// context implementation using merkle-tree-like storage
#[derive(Clone)]
pub struct TezedgeContext {
    block_storage: BlockStorage,
    merkle: Arc<RwLock<MerkleStorage>>,
}

impl TezedgeContext {
    pub fn new(block_storage: BlockStorage, merkle: Arc<RwLock<MerkleStorage>>) -> Self {
        TezedgeContext {
            block_storage,
            merkle,
        }
    }
}

/// Possible errors for context
#[derive(Debug, Fail)]
pub enum ContextError {
    #[fail(
        display = "Failed to assign context_hash: {:?} to block_hash: {}, error: {}",
        context_hash, block_hash, error
    )]
    ContextHashAssignError {
        context_hash: String,
        block_hash: String,
        error: StorageError,
    },
    #[fail(display = "Unknown context_hash: {:?}", context_hash)]
    UnknownContextHashError { context_hash: String },
    #[fail(display = "Unknown level: {}", level)]
    UnknownLevelError { level: String },
    #[fail(display = "Unknown block_hash: {}", block_hash)]
    UnknownBlockHashError { block_hash: String },
    #[fail(display = "Failed operation on Merkle storage: {}", error)]
    MerkleStorageError { error: MerkleError },
    #[fail(display = "Invalid commit date: {}", error)]
    InvalidCommitDate { error: TryFromIntError },
    #[fail(display = "Failed to convert hash to array: {}", error)]
    HashConversionError { error: TryFromSliceError },
    /// TODO: TE-203 - remove when context_listener will not be used
    #[fail(display = "Storage error: {}", error)]
    StorageError { error: StorageError },
    #[fail(display = "Conversion from bytes error: {}", error)]
    HashError { error: FromBytesError },
}

impl From<MerkleError> for ContextError {
    fn from(error: MerkleError) -> Self {
        ContextError::MerkleStorageError { error }
    }
}

impl From<TryFromIntError> for ContextError {
    fn from(error: TryFromIntError) -> Self {
        ContextError::InvalidCommitDate { error }
    }
}

impl From<TryFromSliceError> for ContextError {
    fn from(error: TryFromSliceError) -> Self {
        ContextError::HashConversionError { error }
    }
}

impl From<FromBytesError> for ContextError {
    fn from(error: FromBytesError) -> Self {
        ContextError::HashError { error }
    }
}

/// Marco that simplifies and unificates ContextKey creation
///
/// Common usage:
///
/// `context_key!("protocol")`
/// `context_key!("data/votes/listings")`
/// `context_key!("data/rolls/owner/snapshot/{}/{}", cycle, snapshot)`
/// `context_key!("{}/{}/{}", "data", "votes", "listings")`
///
#[macro_export]
macro_rules! context_key {
    ($key:expr) => {{
        $key.split('/').map(str::to_string).collect::<Vec<String>>()
    }};
    ($($arg:tt)*) => {{
        context_key!(format!($($arg)*))
    }};
}

#[cfg(test)]
mod tests {
    use crate::context_key;

    #[test]
    fn test_context_key_simple() {
        assert_eq!(context_key!("protocol"), vec!["protocol".to_string()],);
    }

    #[test]
    fn test_context_key_mutliple() {
        assert_eq!(
            context_key!("data/votes/listings"),
            vec![
                "data".to_string(),
                "votes".to_string(),
                "listings".to_string()
            ],
        );
    }

    #[test]
    fn test_context_key_format() {
        let cycle: i64 = 5;
        let snapshot: i16 = 9;
        assert_eq!(
            context_key!("data/rolls/owner/snapshot/{}/{}", cycle, snapshot),
            vec![
                "data".to_string(),
                "rolls".to_string(),
                "owner".to_string(),
                "snapshot".to_string(),
                "5".to_string(),
                "9".to_string()
            ],
        );
    }
}
