// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::merkle_storage::{hash_entry, Entry};
use crate::persistent::database::DBError;
use crate::persistent::database::KeyValueStoreBackend;
use crate::MerkleStorage;
use crypto::hash::FromBytesError;
use crypto::hash::HashType;
use failure::Fail;
use serde::Serialize;
use std::array::TryFromSliceError;
use std::collections::HashMap;
use std::collections::HashSet;
use std::mem;
use std::sync::PoisonError;

use crate::merkle_storage::{ContextValue, EntryHash};

pub fn size_of_vec<T>(v: &Vec<T>) -> usize {
    mem::size_of::<Vec<T>>() + mem::size_of::<T>() * v.capacity()
}

pub trait GarbageCollector {
    fn new_cycle_started(&mut self) -> Result<(), StorageBackendError>;

    fn block_applied(&mut self, commit: EntryHash) -> Result<(), StorageBackendError>;
}

pub trait NotGarbageCollected {}

impl<T: NotGarbageCollected> GarbageCollector for T {
    fn new_cycle_started(&mut self) -> Result<(), StorageBackendError> {
        Ok(())
    }

    fn block_applied(&mut self, _commit: EntryHash) -> Result<(), StorageBackendError> {
        Ok(())
    }
}

/// helper function for fetching and deserializing entry from the store
pub fn fetch_entry_from_store(
    store: &dyn KeyValueStoreBackend<MerkleStorage>,
    hash: EntryHash,
) -> Result<Entry, StorageBackendError> {
    match store.get(&hash)? {
        None => Err(StorageBackendError::EntryNotFound {
            hash: HashType::ContextHash.hash_to_b58check(&hash)?,
        }),
        Some(entry_bytes) => Ok(bincode::deserialize(&entry_bytes)?),
    }
}

pub fn collect_hashes_recursively(
    entry: &Entry,
    cache: HashMap<EntryHash, HashSet<EntryHash>>,
    store: &dyn KeyValueStoreBackend<MerkleStorage>,
) -> Result<HashMap<EntryHash, HashSet<EntryHash>>, StorageBackendError> {
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
    store: &dyn KeyValueStoreBackend<MerkleStorage>,
) -> Result<(), StorageBackendError> {
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
                        let entry = fetch_entry_from_store(store, child_node.entry_hash)?;
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
pub enum StorageBackendError {
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
    #[fail(display = "Failed to encode hash: {}", error)]
    HashError { error: FromBytesError },
}

impl From<rocksdb::Error> for StorageBackendError {
    fn from(error: rocksdb::Error) -> Self {
        StorageBackendError::RocksDBError { error }
    }
}

impl From<sled::Error> for StorageBackendError {
    fn from(error: sled::Error) -> Self {
        StorageBackendError::SledDBError { error }
    }
}

impl From<DBError> for StorageBackendError {
    fn from(error: DBError) -> Self {
        StorageBackendError::DBError { error }
    }
}

impl From<bincode::Error> for StorageBackendError {
    fn from(error: bincode::Error) -> Self {
        StorageBackendError::SerializationError { error }
    }
}

impl From<TryFromSliceError> for StorageBackendError {
    fn from(error: TryFromSliceError) -> Self {
        StorageBackendError::HashConversionError { error }
    }
}

impl<T> From<PoisonError<T>> for StorageBackendError {
    fn from(pe: PoisonError<T>) -> Self {
        StorageBackendError::LockError {
            reason: format!("{}", pe),
        }
    }
}

impl From<FromBytesError> for StorageBackendError {
    fn from(error: FromBytesError) -> Self {
        StorageBackendError::HashError { error }
    }
}

impl slog::Value for StorageBackendError {
    fn serialize(
        &self,
        _record: &slog::Record,
        key: slog::Key,
        serializer: &mut dyn slog::Serializer,
    ) -> slog::Result {
        serializer.emit_arguments(key, &format_args!("{}", self))
    }
}

#[derive(Debug, Default, Clone, Copy, Serialize)]
pub struct StorageBackendStats {
    pub key_bytes: usize,
    pub value_bytes: usize,
    pub reused_keys_bytes: usize,
}

impl StorageBackendStats {
    /// increases `reused_keys_bytes` based on `key`
    pub fn update_reused_keys(&mut self, list: &HashSet<EntryHash>) {
        self.reused_keys_bytes = list.capacity() * mem::size_of::<EntryHash>();
    }

    pub fn total_as_bytes(&self) -> usize {
        self.key_bytes + self.value_bytes + self.reused_keys_bytes
    }
}

impl<'a> std::ops::Add<&'a Self> for StorageBackendStats {
    type Output = Self;

    fn add(self, other: &'a Self) -> Self::Output {
        Self {
            key_bytes: self.key_bytes + other.key_bytes,
            value_bytes: self.value_bytes + other.value_bytes,
            reused_keys_bytes: self.reused_keys_bytes + other.reused_keys_bytes,
        }
    }
}

impl std::ops::Add for StorageBackendStats {
    type Output = Self;

    fn add(self, other: Self) -> Self::Output {
        self + &other
    }
}

impl<'a> std::ops::AddAssign<&'a Self> for StorageBackendStats {
    fn add_assign(&mut self, other: &'a Self) {
        *self = *self + other;
    }
}

impl std::ops::AddAssign for StorageBackendStats {
    fn add_assign(&mut self, other: Self) {
        *self = *self + other;
    }
}

impl<'a> std::ops::Sub<&'a Self> for StorageBackendStats {
    type Output = Self;

    fn sub(self, other: &'a Self) -> Self::Output {
        Self {
            key_bytes: self.key_bytes - other.key_bytes,
            value_bytes: self.value_bytes - other.value_bytes,
            reused_keys_bytes: self.reused_keys_bytes - other.reused_keys_bytes,
        }
    }
}

impl std::ops::Sub for StorageBackendStats {
    type Output = Self;

    fn sub(self, other: Self) -> Self::Output {
        self - &other
    }
}

impl<'a> std::ops::SubAssign<&'a Self> for StorageBackendStats {
    fn sub_assign(&mut self, other: &'a Self) {
        *self = *self - other;
    }
}

impl std::ops::SubAssign for StorageBackendStats {
    fn sub_assign(&mut self, other: Self) {
        *self = *self - other;
    }
}

impl<'a> std::iter::Sum<&'a StorageBackendStats> for StorageBackendStats {
    fn sum<I: Iterator<Item = &'a Self>>(iter: I) -> Self {
        iter.fold(StorageBackendStats::default(), |acc, cur| acc + cur)
    }
}

impl From<(&EntryHash, &ContextValue)> for StorageBackendStats {
    fn from((_, value): (&EntryHash, &ContextValue)) -> Self {
        StorageBackendStats {
            key_bytes: mem::size_of::<EntryHash>(),
            value_bytes: size_of_vec(&value),
            reused_keys_bytes: 0,
        }
    }
}
