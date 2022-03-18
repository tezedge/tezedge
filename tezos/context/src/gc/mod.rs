// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Implementation of the garbage collector for the in-memory repository.

use std::array::TryFromSliceError;
use std::sync::PoisonError;

use blake2::digest::InvalidOutputSize;
use thiserror::Error;

use crypto::hash::{ContextHash, FromBytesError};

use crate::hash::HashingError;
use crate::persistent::DBError;

/// Print logs on stdout with the prefix `[tezedge.gc]`
macro_rules! log {
    () => (println!("[tezedge.gc]"));
    ($($arg:tt)*) => ({
        println!("[tezedge.gc] {}", format_args!($($arg)*))
    })
}

/// Print logs on stderr with the prefix `[tezedge.gc]`
macro_rules! elog {
    () => (eprintln!("[tezedge.gc]"));
    ($($arg:tt)*) => ({
        eprintln!("[tezedge.gc] {}", format_args!($($arg)*));
    })
}

pub mod jemalloc;
mod sorted_map;
mod stats;
pub(crate) mod worker;

// TODO: Use const default when the feature is stabilized, it will be in 1.59.0:
// https://github.com/rust-lang/rust/pull/90207
pub type SortedMap<K, V> = sorted_map::SortedMap<K, V, { sorted_map::DEFAULT_CHUNK_SIZE }>;
pub use sorted_map::{Entry, VacantEntry};

pub trait GarbageCollector {
    fn new_cycle_started(&mut self) -> Result<(), GarbageCollectionError>;

    fn block_applied(
        &mut self,
        block_level: u32,
        context_hash: &ContextHash,
    ) -> Result<(), GarbageCollectionError>;
}

pub trait NotGarbageCollected {}

impl<T: NotGarbageCollected> GarbageCollector for T {
    fn new_cycle_started(&mut self) -> Result<(), GarbageCollectionError> {
        Ok(())
    }

    fn block_applied(
        &mut self,
        _block_level: u32,
        _context_hash: &ContextHash,
    ) -> Result<(), GarbageCollectionError> {
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum GarbageCollectionError {
    #[error("Column family {name} is missing")]
    MissingColumnFamily { name: &'static str },
    #[error("Backend Error")]
    BackendError,
    #[error("Guard Poison {error}")]
    GuardPoison { error: String },
    #[error("DBError error: {error:?}")]
    DBError { error: DBError },
    #[error("Failed to convert hash to array: {error}")]
    HashConversionError { error: TryFromSliceError },
    #[error("GarbageCollector error: {error}")]
    GarbageCollectorError { error: String },
    #[error("Mutex/lock lock error! Reason: {reason:?}")]
    LockError { reason: String },
    #[error("Object not found in store: path={path:?} hash={hash:?}")]
    ObjectNotFound { hash: String, path: String },
    #[error("Failed to convert hash into string: {error}")]
    HashToStringError { error: FromBytesError },
    #[error("Failed to encode hash: {error}")]
    HashingError { error: HashingError },
    #[error("Invalid output size")]
    InvalidOutputSize,
    #[error("Expected value instead of `None` for {0}")]
    ValueExpected(&'static str),
    #[error("ContextHash not found: {context_hash}")]
    ContextHashNotFound { context_hash: ContextHash },
}

impl From<DBError> for GarbageCollectionError {
    fn from(error: DBError) -> Self {
        GarbageCollectionError::DBError { error }
    }
}

impl From<TryFromSliceError> for GarbageCollectionError {
    fn from(error: TryFromSliceError) -> Self {
        GarbageCollectionError::HashConversionError { error }
    }
}

impl<T> From<PoisonError<T>> for GarbageCollectionError {
    fn from(pe: PoisonError<T>) -> Self {
        GarbageCollectionError::LockError {
            reason: format!("{}", pe),
        }
    }
}

impl From<FromBytesError> for GarbageCollectionError {
    fn from(error: FromBytesError) -> Self {
        GarbageCollectionError::HashToStringError { error }
    }
}

impl From<InvalidOutputSize> for GarbageCollectionError {
    fn from(_: InvalidOutputSize) -> Self {
        GarbageCollectionError::InvalidOutputSize
    }
}

impl From<HashingError> for GarbageCollectionError {
    fn from(error: HashingError) -> Self {
        Self::HashingError { error }
    }
}
