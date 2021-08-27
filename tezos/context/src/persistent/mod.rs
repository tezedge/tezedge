// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    borrow::Cow,
    io,
    sync::{Arc, PoisonError},
};

use crypto::hash::ContextHash;
use failure::Fail;

use tezos_timing::RepositoryMemoryUsage;

use crate::{
    kv_store::{readonly_ipc::ContextServiceError, HashId, HashIdError, VacantObjectHash},
    working_tree::serializer::DeserializationError,
    ObjectHash,
};

pub trait Flushable {
    fn flush(&self) -> Result<(), failure::Error>;
}

pub trait Persistable {
    fn is_persistent(&self) -> bool;
}

pub trait KeyValueStoreBackend {
    /// Write batch into DB atomically
    ///
    /// # Arguments
    /// * `batch` - WriteBatch containing all batched writes to be written to DB
    fn write_batch(&mut self, batch: Vec<(HashId, Arc<[u8]>)>) -> Result<(), DBError>;
    /// Check if database contains given hash id
    ///
    /// # Arguments
    /// * `hash_id` - HashId, to be checked for existence
    fn contains(&self, hash_id: HashId) -> Result<bool, DBError>;
    /// Mark the HashId as a ContextHash
    ///
    /// # Arguments
    /// * `hash_id` - HashId to mark
    fn put_context_hash(&mut self, hash_id: HashId) -> Result<(), DBError>;
    /// Get the HashId corresponding to the ContextHash
    ///
    /// # Arguments
    /// * `context_hash` - ContextHash to find the HashId
    fn get_context_hash(&self, context_hash: &ContextHash) -> Result<Option<HashId>, DBError>;
    /// Read hash associated with given HashId, if exists.
    ///
    /// # Arguments
    /// * `hash_id` - HashId of the ObjectHash
    fn get_hash(&self, hash_id: HashId) -> Result<Option<Cow<ObjectHash>>, DBError>;
    /// Read value associated with given HashId, if exists.
    ///
    /// # Arguments
    /// * `hash_id` - HashId of the value
    fn get_value(&self, hash_id: HashId) -> Result<Option<Cow<[u8]>>, DBError>;
    /// Find an object to insert a new ObjectHash
    /// Return the object
    fn get_vacant_object_hash(&mut self) -> Result<VacantObjectHash, DBError>;
    /// Manually clear the objects, this should be a no-operation if the implementation
    /// has its own garbage collection
    fn clear_objects(&mut self) -> Result<(), DBError>;
    /// Memory usage
    fn memory_usage(&self) -> RepositoryMemoryUsage;
}

/// Possible errors for schema
#[derive(Debug, Fail)]
pub enum DBError {
    #[fail(display = "Column family {} is missing", name)]
    MissingColumnFamily { name: &'static str },
    #[fail(display = "Database incompatibility {}", name)]
    DatabaseIncompatibility { name: String },
    #[fail(display = "Value already exists {}", key)]
    ValueExists { key: String },
    #[fail(
        display = "Found wrong structure. Was looking for {}, but found {}",
        sought, found
    )]
    FoundUnexpectedStructure { sought: String, found: String },
    #[fail(display = "Guard Poison {} ", error)]
    GuardPoison { error: String },
    #[fail(display = "Mutex/lock lock error! Reason: {}", reason)]
    LockError { reason: String },
    #[fail(display = "I/O error {}", error)]
    IOError { error: io::Error },
    #[fail(display = "MemoryStatisticsOverflow")]
    MemoryStatisticsOverflow,
    #[fail(display = "IPC Context access error: {:?}", reason)]
    IpcAccessError { reason: ContextServiceError },
    #[fail(display = "Missing object: {:?}", hash_id)]
    MissingObject { hash_id: HashId },
    #[fail(display = "Conversion from/to HashId failed")]
    HashIdFailed,
    #[fail(display = "Deserialization error: {:?}", error)]
    DeserializationError { error: DeserializationError },
}

impl From<HashIdError> for DBError {
    fn from(_: HashIdError) -> Self {
        DBError::HashIdFailed
    }
}

impl From<DeserializationError> for DBError {
    fn from(error: DeserializationError) -> Self {
        Self::DeserializationError { error }
    }
}

impl<T> From<PoisonError<T>> for DBError {
    fn from(pe: PoisonError<T>) -> Self {
        DBError::LockError {
            reason: format!("{}", pe),
        }
    }
}

impl From<io::Error> for DBError {
    fn from(error: io::Error) -> Self {
        DBError::IOError { error }
    }
}
