// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! This sub module provides different KV alternatives for context persistence

use std::array::TryFromSliceError;
use std::collections::{HashMap, HashSet};
use std::sync::PoisonError;

use blake2::digest::InvalidOutputSize;
use failure::Fail;

use crypto::hash::{FromBytesError, HashType};

use crate::context::merkle::hash::{hash_entry, HashingError};
use crate::context::merkle::Entry;
use crate::context::{ContextKeyValueStoreSchema, EntryHash};
use crate::persistent::{DBError, KeyValueStoreBackend};

pub mod mark_move_gced;
pub mod mark_sweep_gced;

pub trait GarbageCollector {
    fn new_cycle_started(&mut self) -> Result<(), GarbageCollectionError>;

    fn block_applied(&mut self, commit: EntryHash) -> Result<(), GarbageCollectionError>;
}

pub trait NotGarbageCollected {}

impl<T: NotGarbageCollected> GarbageCollector for T {
    fn new_cycle_started(&mut self) -> Result<(), GarbageCollectionError> {
        Ok(())
    }

    fn block_applied(&mut self, _commit: EntryHash) -> Result<(), GarbageCollectionError> {
        Ok(())
    }
}

/// helper function for fetching and deserializing entry from the store
pub fn fetch_entry_from_store(
    store: &dyn KeyValueStoreBackend<ContextKeyValueStoreSchema>,
    hash: EntryHash,
) -> Result<Entry, GarbageCollectionError> {
    match store.get(&hash)? {
        None => Err(GarbageCollectionError::EntryNotFound {
            hash: HashType::ContextHash.hash_to_b58check(&hash)?,
        }),
        Some(entry_bytes) => Ok(bincode::deserialize(&entry_bytes)?),
    }
}

pub fn collect_hashes_recursively(
    entry: &Entry,
    cache: HashMap<EntryHash, HashSet<EntryHash>>,
    store: &dyn KeyValueStoreBackend<ContextKeyValueStoreSchema>,
) -> Result<HashMap<EntryHash, HashSet<EntryHash>>, GarbageCollectionError> {
    let mut entries = HashSet::new();
    let mut c = cache;
    collect_hashes(entry, &mut entries, &mut c, store)?;
    Ok(c)
}

/// collects entries from tree like structure recursively
pub fn collect_hashes(
    entry: &Entry,
    batch: &mut HashSet<EntryHash>,
    cache: &mut HashMap<EntryHash, HashSet<EntryHash>>,
    store: &dyn KeyValueStoreBackend<ContextKeyValueStoreSchema>,
) -> Result<(), GarbageCollectionError> {
    batch.insert(hash_entry(entry)?);

    match cache.get(&hash_entry(entry)?) {
        // if we know subtree already lets just use it
        Some(v) => {
            batch.extend(v);
            Ok(())
        }
        None => {
            match entry {
                Entry::Blob(_) => Ok(()),
                Entry::Tree(tree) => {
                    // Go through all descendants and gather errors. Remap error if there is a failure
                    // anywhere in the recursion paths. TODO: is revert possible?
                    let mut b = HashSet::new();
                    for (_, child_node) in tree.iter() {
                        let entry =
                            fetch_entry_from_store(store, *child_node.entry_hash)?;
                        collect_hashes(&entry, &mut b, cache, store)?;
                    }
                    cache.insert(hash_entry(entry)?, b.clone());
                    batch.extend(b);
                    Ok(())
                }
                Entry::Commit(commit) => {
                    let entry = fetch_entry_from_store(store, commit.root_hash)?;
                    Ok(collect_hashes(&entry, batch, cache, store)?)
                }
            }
        }
    }
}

#[derive(Debug, Fail)]
pub enum GarbageCollectionError {
    #[fail(display = "RocksDB error: {}", error)]
    RocksDBError { error: rocksdb::Error },
    #[fail(display = "Column family {} is missing", name)]
    MissingColumnFamily { name: &'static str },
    #[fail(display = "Backend Error")]
    BackendError,
    #[fail(display = "SledDB error: {}", error)]
    SledDBError { error: sled::Error },
    #[fail(display = "Guard Poison {} ", error)]
    GuardPoison { error: String },
    #[fail(display = "Serialization error: {:?}", error)]
    SerializationError { error: bincode::Error },
    #[fail(display = "DBError error: {:?}", error)]
    DBError { error: DBError },
    #[fail(display = "Failed to convert hash to array: {}", error)]
    HashConversionError { error: TryFromSliceError },
    #[fail(display = "GarbageCollector error: {}", error)]
    GarbageCollectorError { error: String },
    #[fail(display = "Mutex/lock lock error! Reason: {:?}", reason)]
    LockError { reason: String },
    #[fail(display = "Entry not found in store: {:?}", hash)]
    EntryNotFound { hash: String },
    #[fail(display = "Failed to convert hash into string: {}", error)]
    HashToStringError { error: FromBytesError },
    #[fail(display = "Failed to encode hash: {}", error)]
    HashingError { error: HashingError },
    #[fail(display = "Invalid output size")]
    InvalidOutputSize,
    #[fail(display = "Expected value instead of `None` for {}", _0)]
    ValueExpected(&'static str),
}

impl From<rocksdb::Error> for GarbageCollectionError {
    fn from(error: rocksdb::Error) -> Self {
        GarbageCollectionError::RocksDBError { error }
    }
}

impl From<sled::Error> for GarbageCollectionError {
    fn from(error: sled::Error) -> Self {
        GarbageCollectionError::SledDBError { error }
    }
}

impl From<DBError> for GarbageCollectionError {
    fn from(error: DBError) -> Self {
        GarbageCollectionError::DBError { error }
    }
}

impl From<bincode::Error> for GarbageCollectionError {
    fn from(error: bincode::Error) -> Self {
        GarbageCollectionError::SerializationError { error }
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

impl slog::Value for GarbageCollectionError {
    fn serialize(
        &self,
        _record: &slog::Record,
        key: slog::Key,
        serializer: &mut dyn slog::Serializer,
    ) -> slog::Result {
        serializer.emit_arguments(key, &format_args!("{}", self))
    }
}
