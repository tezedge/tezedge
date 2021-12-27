// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{borrow::Cow, convert::TryFrom, io, sync::PoisonError};

use crypto::hash::ContextHash;
use thiserror::Error;

use tezos_timing::{RepositoryMemoryUsage, SerializeStats};

#[cfg(test)]
use crate::serialize::persistent::AbsoluteOffset;
#[cfg(test)]
use std::sync::Arc;

use crate::{
    kv_store::{readonly_ipc::ContextServiceError, HashId, HashIdError, VacantObjectHash},
    serialize::DeserializationError,
    working_tree::{
        shape::{DirectoryShapeError, DirectoryShapeId, ShapeStrings},
        storage::{DirEntryId, InodeId, Storage},
        string_interner::{StringId, StringInterner},
        working_tree::{MerkleError, WorkingTree},
        Object, ObjectReference,
    },
    ContextError, ContextKeyValueStore, ObjectHash,
};

pub mod file;
pub mod lock;

pub trait Flushable {
    fn flush(&self) -> Result<(), anyhow::Error>;
}

pub trait Persistable {
    fn is_persistent(&self) -> bool;
}

pub trait KeyValueStoreBackend {
    /// Check if database contains given hash id
    ///
    /// # Arguments
    /// * `hash_id` - HashId, to be checked for existence
    fn contains(&self, hash_id: HashId) -> Result<bool, DBError>;
    /// Mark the HashId as a ContextHash
    ///
    /// # Arguments
    /// * `hash_id` - HashId to mark
    fn put_context_hash(&mut self, object_ref: ObjectReference) -> Result<(), DBError>;
    /// Get the HashId corresponding to the ContextHash
    ///
    /// # Arguments
    /// * `context_hash` - ContextHash to find the HashId
    fn get_context_hash(
        &self,
        context_hash: &ContextHash,
    ) -> Result<Option<ObjectReference>, DBError>;
    /// Read hash associated with given HashId, if exists.
    ///
    /// # Arguments
    /// * `hash_id` - HashId of the ObjectHash
    fn get_hash(&self, object_ref: ObjectReference) -> Result<Cow<ObjectHash>, DBError>;
    /// Find an object to insert a new ObjectHash
    /// Return the object
    fn get_vacant_object_hash(&mut self) -> Result<VacantObjectHash, DBError>;
    /// Memory usage
    fn memory_usage(&self) -> RepositoryMemoryUsage;
    /// Returns the strings of the directory shape
    fn get_shape(&self, shape_id: DirectoryShapeId) -> Result<ShapeStrings, DBError>;
    /// Returns the `ShapeId` of this `dir`
    ///
    /// Create a new shape when it doesn't exist.
    /// This returns `None` when a shape cannot be made (currently if one of the
    /// string is > 30 bytes).
    fn make_shape(
        &mut self,
        dir: &[(StringId, DirEntryId)],
    ) -> Result<Option<DirectoryShapeId>, DBError>;
    /// Returns the string associated to this `string_id`.
    ///
    /// The string interner must have been updated with the `synchronize_strings_from` method.
    fn get_str(&self, string_id: StringId) -> Option<Cow<str>>;
    /// Update the repository `StringInterner` to be in sync with `string_interner`.
    fn synchronize_strings_from(&mut self, string_interner: &StringInterner);
    /// Return the object associated to this `object_ref`.
    fn get_object(
        &self,
        object_ref: ObjectReference,
        storage: &mut Storage,
        strings: &mut StringInterner,
    ) -> Result<Object, DBError>;
    /// Return the inode associated to this `object_ref`.
    fn get_inode(
        &self,
        object_ref: ObjectReference,
        storage: &mut Storage,
        strings: &mut StringInterner,
    ) -> Result<InodeId, DBError>;
    /// Return the object bytes associated to this `object_ref`.
    ///
    /// The object bytes will be inserted at the beginning of `buffer`.
    ///
    /// Note that the parameter `buffer` is never resized to a smaller length:
    /// If buffer::len is 100 and the object is 15 bytes, after calling this
    /// method the buffer length will still remains 100.
    /// This method returns a slice, which is the exact object bytes
    /// (&buffer[0..15] in the example).
    ///
    /// It's never resized to avoid calling `Vec::resize(new_len, 0)`, which can be relatively
    /// expensive.
    fn get_object_bytes<'a>(
        &self,
        object_ref: ObjectReference,
        buffer: &'a mut Vec<u8>,
    ) -> Result<&'a [u8], DBError>;
    /// Commit the `working_tree` and returns its `ContextHash` and serialization statistics
    fn commit(
        &mut self,
        working_tree: &WorkingTree,
        parent_commit_ref: Option<ObjectReference>,
        author: String,
        message: String,
        date: u64,
    ) -> Result<(ContextHash, Box<SerializeStats>), DBError>;
    /// Return the `HashId` associated to this `object_ref`
    fn get_hash_id(&self, object_ref: ObjectReference) -> Result<HashId, DBError>;
    /// On restart/reload, the repository contains all strings and their hashes (from the db file)
    /// This method is used to give strings and hashes to the index.
    ///
    /// It should be called only once.
    fn take_strings_on_reload(&mut self) -> Option<StringInterner>;
    /// Make the HashId ready to be commited to disk
    ///
    /// This is used on the persistent context, to avoid commiting unused HashId
    fn make_hash_id_ready_for_commit(&mut self, hash_id: HashId) -> Result<HashId, DBError>;
    /// Simulate a `commit`, by writing data to disk/memory, without computing hash
    #[cfg(test)]
    fn synchronize_data(
        &mut self,
        batch: &[(HashId, Arc<[u8]>)],
        output: &[u8],
    ) -> Result<Option<AbsoluteOffset>, DBError>;
}

/// Possible errors for schema
#[derive(Debug, Error)]
pub enum DBError {
    #[error("Database incompatibility {name}")]
    DatabaseIncompatibility { name: String },
    #[error("Value already exists {key}")]
    ValueExists { key: String },
    #[error("Found wrong structure. Was looking for {sought}, but found {found}")]
    FoundUnexpectedStructure { sought: String, found: String },
    #[error("Guard Poison {error} ")]
    GuardPoison { error: String },
    #[error("Mutex/lock lock error! Reason: {reason}")]
    LockError { reason: String },
    #[error("I/O error {error}")]
    IOError {
        #[from]
        error: io::Error,
    },
    #[error("MemoryStatisticsOverflow")]
    MemoryStatisticsOverflow,
    #[error("IPC Context access error: {reason:?}")]
    IpcAccessError { reason: ContextServiceError },
    #[error("Missing object: {object_ref:?}")]
    MissingObject { object_ref: ObjectReference },
    #[error("Conversion from/to HashId failed")]
    HashIdFailed,
    #[error("Deserialization error: {error:?}")]
    DeserializationError {
        #[from]
        error: DeserializationError,
    },
    #[error("Shape error: {error:?}")]
    ShapeError {
        #[from]
        error: DirectoryShapeError,
    },
    #[error("Context error: {error:?}")]
    ContextError {
        #[from]
        error: Box<ContextError>,
    },
    #[error("Hash not found: {object_ref:?}")]
    HashNotFound { object_ref: ObjectReference },
    #[error("Commit error: {err:?}")]
    CommitError {
        #[from]
        err: Box<MerkleError>,
    },
    #[error("Commit to disk error: {err:?}")]
    CommitToDiskError { err: io::Error },
}

impl From<HashIdError> for DBError {
    fn from(_: HashIdError) -> Self {
        DBError::HashIdFailed
    }
}

impl<T> From<PoisonError<T>> for DBError {
    fn from(pe: PoisonError<T>) -> Self {
        DBError::LockError {
            reason: format!("{}", pe),
        }
    }
}

pub(crate) fn get_commit_hash(
    commit_ref: ObjectReference,
    repo: &ContextKeyValueStore,
) -> Result<ContextHash, ContextError> {
    let commit_hash = repo.get_hash(commit_ref)?;
    let commit_hash = ContextHash::try_from(&commit_hash[..])?;
    Ok(commit_hash)
}
