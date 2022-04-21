// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

// #![forbid(unsafe_code)]

//! TezEdge implementation of the context API and storage for the Tezos economic protocol
//!
//! ```no_compile
//!                                                +----------+
//!                                                |  Commit  |
//!                                                +----+-----+
//!                                                     |
//!                                                     |
//!        +-                                           |
//!        |                                   +--------v---------+
//!        |                                   |    Directory     |
//!        |                                   +--------+---------+
//!        |                                   | rolls  |  votes  |
//!        |                   +---------------+--------+---------+----------+
//!        |                   |                                             |
//!        |                   |                                             |
//!        |                   |                  +-                         |
//!        |                   |                  |                          |
//!    T   |             +-----v-------+          |           +--------------v------------+
//!    R   |             |  Directory  |       S  |           |         Directory         |
//!    E   |             +-------------+       U  |           +-----------------+---------+
//!    E   |             |    next     |       B  |           |current_proposal | ballots |
//!        |             +--+----------+       T  |           +---+-------------+----+----+
//!        |                |                  R  |               |                  |
//!        |                |                  E  |               |                  |
//!        |                |                  E  |               |                  |
//!        |                |                     |               |                  |
//!        |                |                     |               |                  |
//!        |      +---------v-----+               |    +----------v----+       +-----v---------+
//!        |      |   Value (A)   |               |    |   Value (B)   |       |   Value (C)   |
//!        |      +---------------+               |    +---------------+       +---------------+
//!        +-                                     +-
//! ```
//!
//! ## Concepts
//!
//! ### Context
//!
//! The context is the state of the blockchain. It is modified by the operations that come in blocks.
//! It is also versioned, which means that after a new version of the context is committed (saved),
//! the older version is still available.
//!
//! In the Tezos economic protocol, the context is represented as a tree, similar to a file system.
//!
//! The model is similar to how git takes "snapshots" of a directory tree in the file system that
//! can later be retrieved, modified and saved as a new version. Each version of the tree is marked
//! by a "commit" which contains among other information, a timestamp, and a reference to the
//! parent version.
//!
//! ### Directories, values/blobs, and trees
//!
//! In the context tree there are two kinds of objects, directories and values (or BLOBs).
//!
//! Directories are like folders in the file system, and contain inside other children objects,
//! associated to a name. Directories cannot be empty.
//!
//! Values/BLOBs contain serialized data (it is the economic protocol that serializes this data,
//! the context never sees the deserialized version). Values don't contain other objects inside.
//!
//! A tree is the combination of one or more directories and values (a single value is a tree too).
//!
//! ### Checkouts, commits, the commit object, the index and the repository
//!
//! A working tree is obtained by performing a "checkout", and saved by performing a "commit".
//!
//! When a working tree is committed, all the objects that compose it are serialized and saved to the "repository".
//! In addition, a "commit object" which contains a pointer to the root of the tree is created and saved.
//! Commit objects are the entry points to versions of the context tree.
//!
//! The repository is a raw store to which all objects that compose committed trees are saved. Every object
//! that is saved to the repository can be retrieved again by it's hash (the exception is inlined objects).
//! In effect, the repository is a key-value store of which every key is a hash of the object it references.
//! There are many different implementations of the repository.
//!
//! The index is an abstraction over the repository, that among other things, implements the
//! commit and checkout functionality. The index can use different repository implementations.
//!
//! A checkout is made with the hash of a commit object as an input, and from there a specific version
//! of the context tree can be reconstructed.
//!
//! ### Working tree
//!
//! The working tree, is a manipulable, unsaved instance of the context tree. It is obtained after
//! doing a checkout. Once materialized, it can be manipulated, and eventually saved by committing it.
//!
//! ### Working tree storage (TODO: better name)
//!
//! The working tree storage is an ephemeral, pre-allocated block of memory.
//!
//! Objects in the working tree are constructed from this block of memory that keeps all
//! objects packed together to avoid fragmentation. Each time an object is created, a special identifier
//! to represent the object is created and returned.
//!
//! When the working tree is committed, this storage is cleared.
//!
//! The storage is contained in the index, and is shared by all the versions of a working tree
//! that is obtained from a checkout.
//!
//! ### Objects
//!
//! There are two kinds of objects in a tree: directories and values/BLOBs.
//! There are three kinds of objects in the repository: directories, values/BLOBs, and commits.
//!
//! ### Context Protocol and Context Shell API
//!
//! There are two parts of the Context API, one seen by the protocol, containing the functions used to query
//! and manipulate the working tree, and the other part that only the shell can see that exposes the
//! functionality that interacts with the repository (commit and checkout).
//!

/// Print logs on stdout with the prefix `[tezedge.context]`
macro_rules! log {
    () => (println!("[tezedge.context]"));
    ($($arg:tt)*) => ({
        println!("[tezedge.context] {}", format_args!($($arg)*))
    })
}

/// Print logs on stderr with the prefix `[tezedge.context]`
macro_rules! elog {
    () => (eprintln!("[tezedge.context]"));
    ($($arg:tt)*) => ({
        eprintln!("[tezedge.context] {}", format_args!($($arg)*));
    })
}

pub mod chunks;
pub mod gc;
pub mod hash;
pub mod serialize;
pub mod working_tree;

pub mod ffi;
pub mod from_ocaml;
pub mod initializer;
pub mod timings;

pub mod snapshot;

pub fn force_libtezos_linking() {
    tezos_sys::force_libtezos_linking();
}

use std::array::TryFromSliceError;
use std::num::TryFromIntError;
use std::sync::PoisonError;

use gc::GarbageCollectionError;
pub use kv_store::persistent::Persistent;

use persistent::{DBError, KeyValueStoreBackend};
use tezos_context_api::{ContextKey, ContextKeyOwned, ContextValue, StringTreeObject};
use thiserror::Error;

pub use hash::ObjectHash;
pub use tezedge_context::PatchContextFunction;
pub use tezedge_context::TezedgeContext;
pub use tezedge_context::TezedgeIndex;
use tezos_timing::ContextMemoryUsage;
use working_tree::working_tree::FoldOrder;
use working_tree::working_tree::{FoldDepth, TreeWalker, WorkingTree};

use crate::gc::GarbageCollector;
use crate::working_tree::working_tree::MerkleError;
use crypto::hash::{ContextHash, FromBytesError};

pub mod persistent;

use crate::persistent::{Flushable, Persistable};

pub mod kv_store;
pub mod tezedge_context;

/// An unique tree identifier during a block application
pub type TreeId = i32;

/// Context API used by the Protocol to manipulate the working tree
pub trait ProtocolContextApi
where
    Self: Sized,
{
    // set key-value
    fn add(&self, key: &ContextKey, value: &[u8]) -> Result<Self, ContextError>;
    // delete key-value
    fn delete(&self, key_prefix_to_delete: &ContextKey) -> Result<Self, ContextError>;
    // find value for key
    fn find(&self, key: &ContextKey) -> Result<Option<ContextValue>, ContextError>;
    // mem - check if value exists
    fn mem(&self, key: &ContextKey) -> Result<bool, ContextError>;
    // mem_tree - check if tree exists
    fn mem_tree(&self, key: &ContextKey) -> bool;
    fn find_tree(&self, key: &ContextKey) -> Result<Option<WorkingTree>, ContextError>;
    fn add_tree(&self, key: &ContextKey, tree: &WorkingTree) -> Result<Self, ContextError>;
    fn empty(&self) -> Self;
    fn list(
        &self,
        offset: Option<usize>,
        length: Option<usize>,
        key: &ContextKey,
    ) -> Result<Vec<(String, WorkingTree)>, ContextError>;
    fn fold_iter(
        &self,
        depth: Option<FoldDepth>,
        key: &ContextKey,
        order: FoldOrder,
    ) -> Result<TreeWalker, ContextError>;

    fn get_merkle_root(&self) -> Result<ObjectHash, ContextError>;
}

/// Index API used by the Shell
pub trait IndexApi<T: ShellContextApi + ProtocolContextApi> {
    // checks if a commit exists in the repository
    fn exists(&self, context_hash: &ContextHash) -> Result<bool, ContextError>;
    // checkout context for hash
    fn checkout(&self, context_hash: &ContextHash) -> Result<Option<T>, ContextError>;
    // called after a block is applied
    fn block_applied(
        &self,
        block_level: u32,
        context_hash: &ContextHash,
    ) -> Result<(), ContextError>;
    // Return the last context hashes
    // The vector is ordered from oldest to latest context hash
    fn latest_context_hashes(&self, count: i64) -> Result<Vec<ContextHash>, ContextError>;
    // called when a new cycle starts
    fn cycle_started(&mut self) -> Result<(), ContextError>;
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
    ) -> Result<Option<Vec<(ContextKeyOwned, ContextValue)>>, ContextError>;
    // get entire context tree in string form for JSON RPC
    fn get_context_tree_by_prefix(
        &self,
        context_hash: &ContextHash,
        prefix: &ContextKey,
        depth: Option<usize>,
    ) -> Result<StringTreeObject, ContextError>;
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

    // get currently checked out hash
    fn get_last_commit_hash(&self) -> Result<Option<Vec<u8>>, ContextError>;

    fn get_memory_usage(&self) -> Result<ContextMemoryUsage, ContextError>;
}
/// Possible errors for context
#[derive(Debug, Error)]
pub enum ContextError {
    #[error("Unknown context_hash: {context_hash:?} - {object_hash:?}")]
    UnknownContextHashAndObjectError {
        context_hash: String,
        object_hash: String,
    },
    #[error("Unknown context_hash: {context_hash:?}")]
    UnknownContextHashError { context_hash: String },
    #[error("Failed operation on Merkle storage: {error}")]
    MerkleStorageError { error: MerkleError },
    #[error("Invalid commit date: {error}")]
    InvalidCommitDate { error: TryFromIntError },
    #[error("Failed to convert hash to array: {error}")]
    HashConversionError { error: TryFromSliceError },
    #[error("Conversion from bytes error: {error}")]
    HashError { error: FromBytesError },
    #[error("Garbage Collection error {error:?}")]
    GarbageCollectionError { error: GarbageCollectionError },
    #[error("Database error error {error:?}")]
    DBError { error: DBError },
    #[error("Found wrong structure. Was looking for {sought}, but found {found}")]
    FoundUnexpectedStructure { sought: String, found: String },
    #[error("Mutex/lock error, reason: {reason:?}")]
    LockError { reason: String },
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

impl<T> From<PoisonError<T>> for ContextError {
    fn from(pe: PoisonError<T>) -> Self {
        Self::LockError {
            reason: format!("{}", pe),
        }
    }
}

/// Base trait for kv-store to be used with merkle
pub type ContextKeyValueStore = dyn ContextKeyValueStoreWithGargbageCollection + Sync + Send;

pub trait ContextKeyValueStoreWithGargbageCollection:
    KeyValueStoreBackend + GarbageCollector + Flushable + Persistable
{
}

impl<T: KeyValueStoreBackend + GarbageCollector + Flushable + Persistable>
    ContextKeyValueStoreWithGargbageCollection for T
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
        $key.split('/').collect::<Vec<&str>>()

    }};
    ($($arg:tt)*) => {{
        context_key!(format!($($arg)*))
    }};
}

// Like [`context_key`] but produces ann owned key.
#[macro_export]
macro_rules! context_key_owned {
    ($key:expr) => {{
        $key.split('/').map(String::from).collect::<Vec<String>>()
    }};
    ($($arg:tt)*) => {{
        context_key_owned!(format!($($arg)*))
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

#[derive(Clone, Default)]
pub struct NoHash(u64);

impl std::hash::BuildHasher for NoHash {
    type Hasher = Self;
    fn build_hasher(&self) -> Self::Hasher {
        Self(0)
    }
}

impl std::hash::Hasher for NoHash {
    fn finish(&self) -> u64 {
        self.0 as u64
    }
    fn write(&mut self, _slice: &[u8]) {
        panic!("Wrong use of NoHash");
    }
    fn write_usize(&mut self, n: usize) {
        self.0 = n as u64
    }
    fn write_u64(&mut self, n: u64) {
        self.0 = n;
    }
    fn write_u32(&mut self, n: u32) {
        self.0 = n as u64
    }
    fn write_u16(&mut self, n: u16) {
        self.0 = n as u64
    }
    fn write_u8(&mut self, n: u8) {
        self.0 = n as u64
    }
}

/// A map, without hashing
///
/// This is useful when the keys of the map are integers (u8, u32, u64, etc.)
/// Since the keys are already integers, they do not need to be hashed.
/// This `Map` will not call any hashing algorithm.
pub type Map<K, V> = std::collections::BTreeMap<K, V>;
