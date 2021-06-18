// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! This module provides wrapper on RocksDB database.
//! Everything related to RocksDB should be placed here.

use std::io;
use std::sync::PoisonError;

use failure::Fail;

use crypto::hash::FromBytesError;

use crate::kv_store::{readonly_ipc::ContextServiceError, HashId};
use crate::persistent::codec::SchemaError;

/// Possible errors for schema
#[derive(Debug, Fail)]
pub enum DBError {
    #[fail(display = "Schema error: {}", error)]
    SchemaError { error: SchemaError },
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
    #[fail(display = "Serialization error: {:?}", error)]
    SerializationError { error: bincode::Error },
    #[fail(display = "Hash encode error : {}", error)]
    HashEncodeError { error: FromBytesError },
    #[fail(display = "Mutex/lock lock error! Reason: {}", reason)]
    LockError { reason: String },
    #[fail(display = "I/O error {}", error)]
    IOError { error: io::Error },
    #[fail(display = "MemoryStatisticsOverflow")]
    MemoryStatisticsOverflow,
    #[fail(display = "IPC Context access error: {:?}", reason)]
    IpcAccessError { reason: ContextServiceError },
    #[fail(display = "Missing entry: {:?}", hash_id)]
    MissingEntry { hash_id: HashId },
}

impl From<SchemaError> for DBError {
    fn from(error: SchemaError) -> Self {
        DBError::SchemaError { error }
    }
}

impl From<FromBytesError> for DBError {
    fn from(error: FromBytesError) -> Self {
        DBError::HashEncodeError { error }
    }
}

impl From<bincode::Error> for DBError {
    fn from(error: bincode::Error) -> Self {
        Self::SerializationError { error }
    }
}

impl slog::Value for DBError {
    fn serialize(
        &self,
        _record: &slog::Record,
        key: slog::Key,
        serializer: &mut dyn slog::Serializer,
    ) -> slog::Result {
        serializer.emit_arguments(key, &format_args!("{}", self))
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
