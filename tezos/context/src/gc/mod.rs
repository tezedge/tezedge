// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Implementation of the garbage collector for the in-memory repository.

use std::array::TryFromSliceError;
use std::sync::PoisonError;

use blake2::digest::InvalidOutputSize;
use thiserror::Error;

use crypto::hash::FromBytesError;

use crate::persistent::DBError;
use crate::{hash::HashingError, kv_store::HashId};

pub(crate) mod sorted_map;
pub(crate) mod worker;

pub trait GarbageCollector {
    fn new_cycle_started(&mut self) -> Result<(), GarbageCollectionError>;

    fn block_applied(
        &mut self,
        referenced_older_objects: Vec<HashId>,
    ) -> Result<(), GarbageCollectionError>;
}

pub trait NotGarbageCollected {}

impl<T: NotGarbageCollected> GarbageCollector for T {
    fn new_cycle_started(&mut self) -> Result<(), GarbageCollectionError> {
        Ok(())
    }

    fn block_applied(
        &mut self,
        _referenced_older_objects: Vec<HashId>,
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
