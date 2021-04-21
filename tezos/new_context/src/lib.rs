// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

// #![forbid(unsafe_code)]

//! This crate contains code which is used to move context messages between OCaml and Rust worlds.
//!
//! Code in this crate should not reference any other tezedge crates to avoid circular dependencies.
//! At OCaml side message is pushed into crossbeam channel. At Rust side messages are fetched from
//! the crossbeam channel.

pub mod gc;
pub mod hash;
pub mod working_tree;

pub mod ffi;
pub mod from_ocaml;
pub mod initializer;

pub fn force_libtezos_linking() {
    tezos_sys::force_libtezos_linking();
}

use std::collections::BTreeMap;
use std::num::TryFromIntError;
use std::{array::TryFromSliceError, rc::Rc};

use failure::Fail;
use gc::GarbageCollectionError;
use persistent::DBError;
use serde::Deserialize;
use serde::Serialize;

pub use actions::ActionRecorder;
pub use hash::EntryHash;
pub use tezedge_context::TezedgeContext;
pub use tezedge_context::TezedgeIndex;
use working_tree::NodeKind;

use crate::gc::GarbageCollector;
use crate::working_tree::working_tree::MerkleError;
use crate::working_tree::working_tree_stats::MerkleStoragePerfReport;
use crypto::hash::{ContextHash, FromBytesError};

mod persistent;

use crate::persistent::{Flushable, KeyValueSchema, KeyValueStoreBackend, Persistable};

pub mod actions;
pub mod kv_store;
pub mod tezedge_context;

pub type ContextKey = Vec<String>;
pub type ContextValue = Vec<u8>;

/// An unique tree identifier during a block application
pub type TreeId = i32;

/// Tree in String form needed for JSON RPCs
pub type StringTreeMap = BTreeMap<Rc<String>, StringTreeEntry>;

/// Tree in String form needed for JSON RPCs
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StringTreeEntry {
    Tree(StringTreeMap),
    Blob(String),
    Null,
}

/// Context API used by the Protocol to manipulate the working tree
pub trait ProtocolContextApi
where
    Self: Sized,
{
    // set key-value
    fn add(&self, key: &ContextKey, value: ContextValue) -> Result<Self, ContextError>;
    // delete key-value
    fn delete(&self, key_prefix_to_delete: &ContextKey) -> Result<Self, ContextError>;
    // copies subtree under 'from_key' to new subtree under 'to_key', returns None if from_key doesn't exist
    fn copy(
        &self,
        from_key: &ContextKey,
        to_key: &ContextKey,
    ) -> Result<Option<Self>, ContextError>;
    // find value for key
    fn find(&self, key: &ContextKey) -> Result<Option<ContextValue>, ContextError>;
    // mem - check if value exists
    fn mem(&self, key: &ContextKey) -> Result<bool, ContextError>;
    // mem_tree - check if directory exists
    fn mem_tree(&self, key: &ContextKey) -> bool;
    fn find_tree(&self, key: &ContextKey) -> Result<Option<Self>, MerkleError>;
    fn add_tree(&self, key: &ContextKey, tree: &Self) -> Result<Self, MerkleError>;
    fn equal(&self, other: &Self) -> Result<bool, MerkleError>;
    fn hash(&self) -> Result<EntryHash, MerkleError>;
    fn kind(&self, key: &ContextKey) -> Result<NodeKind, MerkleError>;
    fn empty(&self) -> Self;
    fn is_empty(&self) -> bool;
    fn list(&self, key: &ContextKey);
    fn fold(&self, key: &ContextKey);

    fn get_merkle_root(&self) -> Result<EntryHash, ContextError>;
}

/// Index API used by the Shell
pub trait IndexApi<T: ShellContextApi + ProtocolContextApi> {
    // checks if a commit exists in the repository
    fn exists(&self, context_hash: &ContextHash) -> Result<bool, ContextError>;
    // checkout context for hash
    fn checkout(&self, context_hash: &ContextHash) -> Result<Option<T>, ContextError>;
}

/// Context API used by the Shell
pub trait ShellContextApi
where
    Self: Sized,
{
    // commit current context diff to storage
    fn commit(
        &self,
        author: String,
        message: String,
        date: i64,
    ) -> Result<ContextHash, ContextError>;
    fn hash(&self, author: String, message: String, date: i64)
        -> Result<ContextHash, ContextError>;
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
    ) -> Result<Option<Vec<(ContextKey, ContextValue)>>, ContextError>;
    // get entire context tree in string form for JSON RPC
    fn get_context_tree_by_prefix(
        &self,
        context_hash: &ContextHash,
        prefix: &ContextKey,
        depth: Option<usize>,
    ) -> Result<StringTreeEntry, ContextError>;

    // get currently checked out hash
    fn get_last_commit_hash(&self) -> Result<Option<Vec<u8>>, ContextError>;
    // get stats from merkle storage
    fn get_merkle_stats(&self) -> Result<MerkleStoragePerfReport, ContextError>;

    fn block_applied(&mut self, last_commit_hash: ContextHash) -> Result<(), ContextError>;

    fn cycle_started(&mut self) -> Result<(), ContextError>;

    fn get_memory_usage(&self) -> Result<usize, ContextError>;
}
/// Possible errors for context
#[derive(Debug, Fail)]
pub enum ContextError {
    #[fail(display = "Unknown context_hash: {:?}", context_hash)]
    UnknownContextHashError { context_hash: String },
    #[fail(display = "Failed operation on Merkle storage: {}", error)]
    MerkleStorageError { error: MerkleError },
    #[fail(display = "Invalid commit date: {}", error)]
    InvalidCommitDate { error: TryFromIntError },
    #[fail(display = "Failed to convert hash to array: {}", error)]
    HashConversionError { error: TryFromSliceError },
    #[fail(display = "Conversion from bytes error: {}", error)]
    HashError { error: FromBytesError },
    #[fail(display = "Garbage Collection error {:?}", error)]
    GarbageCollectionError { error: GarbageCollectionError },
    #[fail(display = "Database error error {:?}", error)]
    DBError { error: DBError },
    #[fail(display = "Serialization error: {:?}", error)]
    SerializationError { error: bincode::Error },
    #[fail(
        display = "Found wrong structure. Was looking for {}, but found {}",
        sought, found
    )]
    FoundUnexpectedStructure { sought: String, found: String },
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

impl From<GarbageCollectionError> for ContextError {
    fn from(error: GarbageCollectionError) -> Self {
        ContextError::GarbageCollectionError { error }
    }
}

impl From<DBError> for ContextError {
    fn from(error: DBError) -> Self {
        ContextError::DBError { error }
    }
}

impl From<bincode::Error> for ContextError {
    fn from(error: bincode::Error) -> Self {
        Self::SerializationError { error }
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
    KeyValueStoreBackend<ContextKeyValueStoreSchema> + GarbageCollector + Flushable + Persistable
{
}

impl<
        T: KeyValueStoreBackend<ContextKeyValueStoreSchema>
            + GarbageCollector
            + Flushable
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
