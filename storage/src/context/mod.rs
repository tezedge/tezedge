// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::array::TryFromSliceError;
use std::collections::BTreeMap;
use std::num::TryFromIntError;
use std::sync::PoisonError;

use failure::Fail;
use serde::Deserialize;
use serde::Serialize;

pub use actions::ActionRecorder;
use crypto::hash::{BlockHash, ContextHash, FromBytesError};
pub use merkle::hash::EntryHash;
pub use tezedge_context::TezedgeContext;
use tezos_context::channel::ContextAction;

use crate::context::gc::GarbageCollector;
use crate::context::merkle::merkle_storage::MerkleError;
use crate::context::merkle::merkle_storage_stats::MerkleStoragePerfReport;
use crate::persistent::{
    Flushable, KeyValueSchema, KeyValueStoreBackend, MultiInstanceable, Persistable,
};
use crate::StorageError;

pub mod actions;
pub mod gc;
pub mod kv_store;
pub mod merkle;
pub mod tezedge_context;

pub type ContextKey = Vec<String>;
pub type ContextValue = Vec<u8>;

/// An unique tree identifier during a block application
pub type TreeId = i32;

/// Tree in String form needed for JSON RPCs
pub type StringTreeMap = BTreeMap<String, StringTreeEntry>;

/// Tree in String form needed for JSON RPCs
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StringTreeEntry {
    Tree(StringTreeMap),
    Blob(String),
    Null,
}

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

    fn perform_context_action(&mut self, action: &ContextAction) -> Result<(), failure::Error>;
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
    #[fail(display = "Guard Poison {} ", error)]
    LockError { error: String },
    #[fail(display = "Cannot check if context is commited")]
    CommitStatusCheckFailure,
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

impl<T> From<PoisonError<T>> for ContextError {
    fn from(pe: PoisonError<T>) -> Self {
        ContextError::LockError {
            error: format!("{}", pe),
        }
    }
}

// keys is hash of Entry
pub type ContextKeyValueStoreSchemaKeyType = EntryHash;
// Entry (serialized) - watch out, this is not the same as ContextValue
pub type MerkleKeyValueStoreSchemaValueType = Vec<u8>;

/// Common serialization prescript for K-V
pub struct ContextKeyValueStoreSchema;

impl KeyValueSchema for ContextKeyValueStoreSchema {
    type Key = ContextKeyValueStoreSchemaKeyType;
    type Value = MerkleKeyValueStoreSchemaValueType;
}

/// Base trait for kv-store to be used with merkle
pub type ContextKeyValueStore = dyn ContextKeyValueStoreWithGargbageCollection + Sync + Send;

pub trait ContextKeyValueStoreWithGargbageCollection:
    KeyValueStoreBackend<ContextKeyValueStoreSchema>
    + GarbageCollector
    + Flushable
    + MultiInstanceable
    + Persistable
{
}

impl<
        T: KeyValueStoreBackend<ContextKeyValueStoreSchema>
            + GarbageCollector
            + Flushable
            + MultiInstanceable
            + Persistable,
    > ContextKeyValueStoreWithGargbageCollection for T
{
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
