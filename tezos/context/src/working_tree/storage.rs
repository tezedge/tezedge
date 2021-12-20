// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! The "storage" is where all data used in the working tree is allocated and stored.
//! Data is represented in a flat form and kept in a contiguous memory area, which is more compact
//! and avoids memory fragmentation. Instead of pointers, special IDs encoding extra information
//! are used as references to different types of values.

use std::{
    borrow::Cow,
    cell::Cell,
    cmp::Ordering,
    collections::HashMap,
    convert::{TryFrom, TryInto},
    mem::size_of,
    ops::{Range, RangeInclusive},
};

use modular_bitfield::prelude::*;
use static_assertions::assert_eq_size;
use tezos_timing::StorageMemoryUsage;
use thiserror::Error;

use crate::{
    chunks::ChunkedVec,
    hash::HashingError,
    kv_store::{index_map::IndexMap, HashId},
    working_tree::ObjectReference,
    ContextKeyValueStore,
};
use crate::{hash::index as index_of_key, serialize::persistent::AbsoluteOffset};

use super::{
    string_interner::{StringId, StringInterner},
    working_tree::MerkleError,
    DirEntry,
};

/// Threshold when a 'small' directory must become an `Inode` (and reverse)
const DIRECTORY_INODE_THRESHOLD: usize = 256;

/// Threshold when a `Inode::Directory` must be converted to a another `Inode::Pointers`
const INODE_POINTER_THRESHOLD: usize = 32;

// Bitsmaks used on ids/indexes
const FULL_60_BITS: usize = 0xFFFFFFFFFFFFFFF;
const FULL_56_BITS: usize = 0xFFFFFFFFFFFFFF;
const FULL_32_BITS: usize = 0xFFFFFFFF;
const FULL_31_BITS: usize = 0x7FFFFFFF;
const FULL_28_BITS: usize = 0xFFFFFFF;
const FULL_4_BITS: usize = 0xF;

/// Length of a blob we consider inlined.
///
/// Do not consider blobs of length zero as inlined, this never
/// happens when the dir_entry is running and fix a serialization issue
/// during testing/fuzzing
const BLOB_INLINED_RANGE: RangeInclusive<usize> = 1..=7;

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct DirectoryId {
    /// Note: Must fit in DirEntryInner.object_id (61 bits)
    ///
    /// | 3 bits |  1 bit   | 60 bits |
    /// |--------|----------|---------|
    /// | empty  | is_inode | value   |
    ///
    /// value not inode:
    /// | 32 bits | 28 bits |
    /// |---------|---------|
    /// | start   | length  |
    ///
    /// value inode:
    /// | 60 bits    |
    /// |------------|
    /// | an InodeId |
    ///
    /// Note that the `InodeId` here can only be the root of an `Inode`.
    /// A `DirectoryId` never contains an `InodeId` other than a root.
    /// The working tree doesn't have knowledge of inodes, it's an implementation
    /// detail of the `Storage`
    bits: u64,
}

impl Default for DirectoryId {
    fn default() -> Self {
        Self::empty()
    }
}

impl DirectoryId {
    fn try_new_dir(start: usize, end: usize) -> Result<Self, StorageError> {
        let length = end
            .checked_sub(start)
            .ok_or(StorageError::DirInvalidStartEnd)?;

        if start & !FULL_32_BITS != 0 {
            // Must fit in 32 bits
            return Err(StorageError::DirStartTooBig);
        }

        if length & !FULL_28_BITS != 0 {
            // Must fit in 28 bits
            return Err(StorageError::DirLengthTooBig);
        }

        let dir_id = Self {
            bits: (start as u64) << 28 | length as u64,
        };

        debug_assert_eq!(dir_id.get(), (start as usize, end));

        Ok(dir_id)
    }

    fn try_new_inode(index: usize) -> Result<Self, StorageError> {
        if index & !FULL_60_BITS != 0 {
            // Must fit in 60 bits
            return Err(StorageError::InodeIndexTooBig);
        }

        Ok(Self {
            bits: 1 << 60 | index as u64,
        })
    }

    pub fn is_inode(&self) -> bool {
        self.bits >> 60 != 0
    }

    pub fn get_inode_id(self) -> Option<InodeId> {
        if self.is_inode() {
            Some(InodeId(self.get_inode_index() as u32))
        } else {
            None
        }
    }

    fn get(self) -> (usize, usize) {
        debug_assert!(!self.is_inode());

        let start = (self.bits as usize) >> FULL_28_BITS.count_ones();
        let length = (self.bits as usize) & FULL_28_BITS;

        (start, start + length)
    }

    /// Return the length of the small directory.
    ///
    /// Use `Storage::dir_len` to get the length of all directories (including inodes)
    fn small_dir_len(self) -> usize {
        debug_assert!(!self.is_inode());

        (self.bits as usize) & FULL_28_BITS
    }

    fn get_inode_index(self) -> usize {
        debug_assert!(self.is_inode());

        self.bits as usize & FULL_60_BITS
    }

    pub fn empty() -> Self {
        // Never fails
        Self::try_new_dir(0, 0).unwrap()
    }

    pub fn is_empty(&self) -> bool {
        if self.is_inode() {
            return false;
        }

        self.small_dir_len() == 0
    }
}

impl From<DirectoryId> for u64 {
    fn from(dir_id: DirectoryId) -> Self {
        dir_id.bits
    }
}

impl From<u64> for DirectoryId {
    fn from(object_id: u64) -> Self {
        Self { bits: object_id }
    }
}

impl From<InodeId> for DirectoryId {
    fn from(inode_id: InodeId) -> Self {
        // Never fails, `InodeId` is 31 bits, `DirectoryId` expects 60 bits max
        Self::try_new_inode(inode_id.0 as usize).unwrap()
    }
}

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("BlobSliceTooBig: The slice received is too big to be inlined")]
    BlobSliceTooBig,
    #[error("BlobStartTooBig: The start index of a Blob must fit in 32 bits")]
    BlobStartTooBig,
    #[error("BlobLengthTooBig: The length of a Blob must fit in 28 bits")]
    BlobLengthTooBig,
    #[error("DirInvalidStartEnd: The start index of a Dir must not be higher than the end")]
    DirInvalidStartEnd,
    #[error("DirStartTooBig: The start index of a Dir must fit in 32 bits")]
    DirStartTooBig,
    #[error("DirLengthTooBig: The length of a Blob must fit in 28 bits")]
    DirLengthTooBig,
    #[error("InodeIndexTooBig: The Inode index must fit in 31 bits")]
    InodeIndexTooBig,
    #[error("DirEntryIdError: Conversion from/to usize of a DirEntryId failed")]
    DirEntryIdError,
    #[error("StringNotFound: String has not been found")]
    StringNotFound,
    #[error("DirNotFound: Dir has not been found")]
    DirNotFound,
    #[error("BlobNotFound: Blob has not been found")]
    BlobNotFound,
    #[error("DirEntryNotFound: DirEntry has not been found")]
    DirEntryNotFound,
    #[error("InodeNotFound: Inode has not been found")]
    InodeNotFound,
    #[error("ExpectedDirGotInode: Expected a Dir but got an Inode")]
    ExpectedDirGotInode,
    #[error("IterationError: Iteration on an Inode failed")]
    IterationError,
    #[error("RootOfInodeNotAPointer: The root of an Inode must be a pointer")]
    RootOfInodeNotAPointer,
}

impl From<DirEntryIdError> for StorageError {
    fn from(_: DirEntryIdError) -> Self {
        Self::DirEntryIdError
    }
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct BlobId {
    /// Note: Must fit in DirEntryInner.object_id (61 bits)
    ///
    /// | 3 bits  | 1 bit     | 60 bits |
    /// |---------|-----------|---------|
    /// | empty   | is_inline | value   |
    ///
    /// value inline:
    /// | 4 bits | 56 bits |
    /// |--------|---------|
    /// | length | value   |
    ///
    /// value not inline:
    /// | 32 bits | 28 bits |
    /// |---------|---------|
    /// | start   | length  |
    bits: u64,
}

impl From<BlobId> for u64 {
    fn from(blob_id: BlobId) -> Self {
        blob_id.bits
    }
}

impl From<u64> for BlobId {
    fn from(object: u64) -> Self {
        Self { bits: object }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum BlobRef {
    Inline { length: u8, value: [u8; 7] },
    Ref { start: usize, end: usize },
}

impl BlobId {
    fn try_new_inline(value: &[u8]) -> Result<Self, StorageError> {
        let len = value.len();

        // Inline values are 7 bytes maximum
        if len > 7 {
            return Err(StorageError::BlobSliceTooBig);
        }

        // We copy the slice into an array so we can use u64::from_ne_bytes
        let mut new_value: [u8; 8] = [0; 8];
        new_value[..len].copy_from_slice(value);
        let value = u64::from_ne_bytes(new_value);

        let blob_id = Self {
            bits: (1 << 60) | (len as u64) << 56 | value,
        };

        debug_assert_eq!(
            blob_id.get(),
            BlobRef::Inline {
                length: len.try_into().unwrap(),
                value: new_value[..7].try_into().unwrap()
            }
        );

        Ok(blob_id)
    }

    fn try_new(start: usize, end: usize) -> Result<Self, StorageError> {
        let length = end - start;

        if start & !FULL_32_BITS != 0 {
            // Start must fit in 32 bits
            return Err(StorageError::BlobStartTooBig);
        }

        if length & !FULL_28_BITS != 0 {
            // Length must fit in 28 bits
            return Err(StorageError::BlobLengthTooBig);
        }

        let blob_id = Self {
            bits: (start as u64) << 28 | length as u64,
        };

        debug_assert_eq!(blob_id.get(), BlobRef::Ref { start, end });

        Ok(blob_id)
    }

    fn get(self) -> BlobRef {
        if self.is_inline() {
            let length = ((self.bits >> 56) & FULL_4_BITS as u64) as u8;

            // Extract the inline value and make it a slice
            let value: u64 = self.bits & FULL_56_BITS as u64;
            let value: [u8; 8] = value.to_ne_bytes();
            let value: [u8; 7] = value[..7].try_into().unwrap(); // Never fails, `value` is [u8; 8]

            BlobRef::Inline { length, value }
        } else {
            let start = (self.bits >> FULL_28_BITS.count_ones()) as usize;
            let length = self.bits as usize & FULL_28_BITS;

            BlobRef::Ref {
                start,
                end: start + length,
            }
        }
    }

    pub fn is_inline(self) -> bool {
        self.bits >> 60 != 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DirEntryId(u32);

#[derive(Debug, Error)]
#[error("Fail to convert the dir entry id to/from usize")]
pub struct DirEntryIdError;

impl TryInto<usize> for DirEntryId {
    type Error = DirEntryIdError;

    fn try_into(self) -> Result<usize, Self::Error> {
        Ok(self.0 as usize)
    }
}

impl TryFrom<usize> for DirEntryId {
    type Error = DirEntryIdError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        value
            .try_into()
            .map(DirEntryId)
            .map_err(|_| DirEntryIdError)
    }
}

#[derive(Clone, Debug, Copy, PartialEq, Eq)]
pub struct InodeId(u32);

impl TryInto<usize> for InodeId {
    type Error = DirEntryIdError;

    fn try_into(self) -> Result<usize, Self::Error> {
        Ok(self.0 as usize)
    }
}

impl TryFrom<usize> for InodeId {
    type Error = DirEntryIdError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        value.try_into().map(InodeId).map_err(|_| DirEntryIdError)
    }
}

#[bitfield]
#[derive(Clone, Copy, Debug)]
pub struct PointerToInodeInner {
    hash_id: B48,
    is_commited: bool,
    inode_id: B31,
    /// Set to `0` when the offset is not set
    offset: B64,
}

#[derive(Clone, Debug)]
pub struct PointerToInode {
    inner: Cell<PointerToInodeInner>,
}

impl PointerToInode {
    pub fn new(hash_id: Option<HashId>, inode_id: InodeId) -> Self {
        Self {
            inner: Cell::new(
                PointerToInodeInner::new()
                    .with_hash_id(hash_id.map(|h| h.as_u64()).unwrap_or(0))
                    .with_is_commited(false)
                    .with_inode_id(inode_id.0)
                    .with_offset(0),
            ),
        }
    }

    pub fn new_commited(
        hash_id: Option<HashId>,
        inode_id: InodeId,
        offset: Option<AbsoluteOffset>,
    ) -> Self {
        Self {
            inner: Cell::new(
                PointerToInodeInner::new()
                    .with_hash_id(hash_id.map(|h| h.as_u64()).unwrap_or(0))
                    .with_is_commited(true)
                    .with_inode_id(inode_id.0)
                    .with_offset(offset.map(|o| o.as_u64()).unwrap_or(0)),
            ),
        }
    }

    pub fn with_offset(self, offset: u64) -> Self {
        debug_assert_ne!(offset, 0);

        let mut inner = self.inner.get();
        inner.set_offset(offset);
        self.inner.set(inner);

        self
    }

    pub fn inode_id(&self) -> InodeId {
        let inner = self.inner.get();
        let inode_id = inner.inode_id();

        InodeId(inode_id)
    }

    pub fn hash_id(
        &self,
        storage: &Storage,
        repository: &ContextKeyValueStore,
    ) -> Result<Option<HashId>, HashingError> {
        let mut inner = self.inner.get();

        if let Some(hash_id) = HashId::new(inner.hash_id()) {
            return Ok(Some(hash_id));
        };

        let offset = match self.get_offset() {
            Some(offset) => offset,
            None => return Ok(None),
        };

        let hash_id = match storage.offsets_to_hash_id.get(&offset) {
            Some(hash_id) => *hash_id,
            None => {
                let object_ref = ObjectReference::new(None, Some(offset));
                repository.get_hash_id(object_ref)?
            }
        };

        inner.set_hash_id(hash_id.as_u64());
        self.inner.set(inner);

        Ok(Some(hash_id))
    }

    pub fn set_hash_id(&self, hash_id: Option<HashId>) {
        let mut inner = self.inner.get();
        inner.set_hash_id(hash_id.map(|h| h.as_u64()).unwrap_or(0));

        self.inner.set(inner);
    }

    pub fn set_offset(&self, offset: AbsoluteOffset) {
        debug_assert_ne!(offset.as_u64(), 0);

        let mut inner = self.inner.get();
        inner.set_offset(offset.as_u64());

        self.inner.set(inner);
    }

    pub fn get_offset(&self) -> Option<AbsoluteOffset> {
        let inner = self.inner.get();
        let offset: u64 = inner.offset();

        if offset != 0 {
            Some(offset.into())
        } else {
            None
        }
    }

    pub fn is_commited(&self) -> bool {
        let inner = self.inner.get();
        inner.is_commited()
    }
}

assert_eq_size!([u8; 19], Option<PointerToInode>);

/// Inode representation used for hashing directories with > DIRECTORY_INODE_THRESHOLD entries.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug)]
pub enum Inode {
    /// Directory is a list of (StringId, DirEntryId)
    Directory(DirectoryId),
    Pointers {
        depth: u32,
        nchildren: u32,
        npointers: u8,
        /// List of pointers to Inode
        /// When the pointer is `None`, it means that there is no entries
        /// under that index.
        pointers: [Option<PointerToInode>; 32],
    },
}

assert_eq_size!([u8; 624], Inode);

/// A range inside `Storage::temp_dir`
type TempDirRange = Range<usize>;

/// `Storage` contains all the data from the working tree.
///
/// This is where all directories/blobs/strings are allocated.
/// The working tree only has access to ids which refer to data inside `Storage`.
///
/// Because `Storage` is for the working tree only, it is cleared before
/// every checkout.
pub struct Storage {
    /// An map `DirEntryId -> DirEntry`
    nodes: IndexMap<DirEntryId, DirEntry>,
    /// Concatenation of all directories in the working tree.
    /// The working tree has `DirectoryId` which refers to a subslice of this
    /// vector `directories`
    directories: ChunkedVec<(StringId, DirEntryId)>,
    /// Temporary directory, this is used to avoid allocations when we
    /// manipulate `directories`
    /// For example, `Storage::insert` will create a new directory in `temp_dir`, once
    /// done it will copy that directory from `temp_dir` into the end of `directories`
    temp_dir: Vec<(StringId, DirEntryId)>,
    /// Concatenation of all blobs in the working tree.
    /// The working tree has `BlobId` which refers to a subslice of this
    /// vector `blobs`.
    /// Note that blobs < 8 bytes are not included in this vector `blobs`, such
    /// blob is directly inlined in the `BlobId`
    blobs: ChunkedVec<u8>,
    /// Concatenation of all inodes.
    /// Note that the implementation of `Storage` attempt to hide as much as
    /// possible the existence of inodes to the working tree.
    /// The working tree doesn't manipulate `InodeId` but `DirectoryId` only.
    /// A `DirectoryId` might contains an `InodeId` but it's only the root
    /// of an Inode, any children of that root are not visible to the working tree.
    inodes: IndexMap<InodeId, Inode>,
    /// Objects bytes are read from disk into this vector
    pub data: Vec<u8>,
    /// Map of deserialized (from disk) offset to their `HashId`.
    pub offsets_to_hash_id: HashMap<AbsoluteOffset, HashId>,
}

#[derive(Debug)]
pub enum Blob<'a> {
    Inline { length: u8, value: [u8; 7] },
    Ref { blob: &'a [u8] },
    Owned { blob: Vec<u8> },
}

impl<'a> AsRef<[u8]> for Blob<'a> {
    fn as_ref(&self) -> &[u8] {
        match self {
            Blob::Inline { length, value } => &value[..*length as usize],
            Blob::Ref { blob } => blob,
            Blob::Owned { blob } => blob,
        }
    }
}

impl<'a> std::ops::Deref for Blob<'a> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

assert_eq_size!([u32; 2], (StringId, DirEntryId));

impl Default for Storage {
    fn default() -> Self {
        Self::new()
    }
}

// Whether or not a non-existing key was added to the inode
type IsNewKey = bool;

const DEFAULT_DIRECTORIES_CAPACITY: usize = 64 * 1024;
const DEFAULT_BLOBS_CAPACITY: usize = 128 * 1024;
const DEFAULT_NODES_CAPACITY: usize = 16 * 1024;
const DEFAULT_INODES_CAPACITY: usize = 256;

impl Storage {
    pub fn new() -> Self {
        Self {
            directories: ChunkedVec::with_chunk_capacity(DEFAULT_DIRECTORIES_CAPACITY), // ~524KB
            temp_dir: Vec::with_capacity(128),                                          // 128B
            blobs: ChunkedVec::with_chunk_capacity(DEFAULT_BLOBS_CAPACITY),             // ~128KB
            nodes: IndexMap::with_chunk_capacity(DEFAULT_NODES_CAPACITY),               // ~360KB
            inodes: IndexMap::with_chunk_capacity(DEFAULT_INODES_CAPACITY),             // ~160KB
            data: Vec::with_capacity(100_000),                                          // ~97KB
            offsets_to_hash_id: HashMap::default(),
        } // Total ~1269KB
    }

    pub fn memory_usage(&self, strings: &StringInterner) -> StorageMemoryUsage {
        let nodes_cap = self.nodes.capacity();
        let directories_cap = self.directories.capacity();
        let blobs_cap = self.blobs.capacity();
        let temp_dir_cap = self.temp_dir.capacity();
        let inodes_cap = self.inodes.capacity();
        let strings = strings.memory_usage();
        let total_bytes = (nodes_cap * size_of::<DirEntry>())
            .saturating_add(directories_cap * size_of::<(StringId, DirEntryId)>())
            .saturating_add(temp_dir_cap * size_of::<(StringId, DirEntryId)>())
            .saturating_add(blobs_cap)
            .saturating_add(inodes_cap * size_of::<Inode>());

        StorageMemoryUsage {
            nodes_len: self.nodes.len(),
            nodes_cap,
            directories_len: self.directories.len(),
            directories_cap,
            temp_dir_cap,
            blobs_len: self.blobs.len(),
            blobs_cap,
            inodes_len: self.inodes.len(),
            inodes_cap,
            strings,
            total_bytes,
        }
    }

    pub fn add_blob_by_ref(&mut self, blob: &[u8]) -> Result<BlobId, StorageError> {
        if BLOB_INLINED_RANGE.contains(&blob.len()) {
            BlobId::try_new_inline(blob)
        } else {
            let (start, length) = self.blobs.extend_from_slice(blob);

            BlobId::try_new(start, start + length)
        }
    }

    pub fn get_blob(&self, blob_id: BlobId) -> Result<Blob, StorageError> {
        match blob_id.get() {
            BlobRef::Inline { length, value } => Ok(Blob::Inline { length, value }),
            BlobRef::Ref { start, end } => match self.blobs.get_slice(start..end) {
                Some(Cow::Borrowed(blob)) => Ok(Blob::Ref { blob }),
                Some(Cow::Owned(blob)) => Ok(Blob::Owned { blob }),
                None => Err(StorageError::BlobNotFound),
            },
        }
    }

    pub fn get_dir_entry(&self, dir_entry_id: DirEntryId) -> Result<&DirEntry, StorageError> {
        self.nodes
            .get(dir_entry_id)?
            .ok_or(StorageError::DirEntryNotFound)
    }

    pub fn add_dir_entry(&mut self, dir_entry: DirEntry) -> Result<DirEntryId, DirEntryIdError> {
        self.nodes.push(dir_entry).map_err(|_| DirEntryIdError)
    }

    /// Return the small directory `dir_id`.
    ///
    /// This returns an error when the underlying directory is an Inode.
    /// To iterate/access all directories (including inodes), `Self::dir_iterate_unsorted`
    /// and `Self::dir_to_vec_unsorted` must be used.
    pub fn get_small_dir(
        &self,
        dir_id: DirectoryId,
    ) -> Result<Cow<[(StringId, DirEntryId)]>, StorageError> {
        if dir_id.is_inode() {
            return Err(StorageError::ExpectedDirGotInode);
        }

        let (start, end) = dir_id.get();
        self.directories
            .get_slice(start..end)
            .ok_or(StorageError::DirNotFound)
    }

    /// [test only] Return the directory with owned values
    #[cfg(test)]
    pub fn get_owned_dir(
        &self,
        dir_id: DirectoryId,
        strings: &StringInterner,
    ) -> Option<Vec<(String, DirEntry)>> {
        let dir = self.dir_to_vec_sorted(dir_id, strings).unwrap();

        Some(
            dir.iter()
                .flat_map(|t| {
                    let key = strings.get_str(t.0).ok()?;
                    let dir_entry = self.nodes.get(t.1).ok()??;
                    Some((key.to_string(), dir_entry.clone()))
                })
                .collect(),
        )
    }

    /// Search `key` in the directory and return the result of `slice::binary_search_by`.
    ///
    /// `dir` must be sorted.
    ///
    /// This returns two `Result<_, _>`:
    /// - The first result (`Result<_, StorageError>`) is an error when something failed
    ///   while reading a string (StorageError::StringNotFound).
    /// - The second result (`Result<usize, usize>`) is from `slice::binary_search_by`.
    ///   If the key was found in the directory, the result is `Ok(Ok(N))`.
    ///   If the key doesn't exist in the directory, the result is `Ok(Err(N))`.
    ///
    ///   see https://doc.rust-lang.org/std/primitive.slice.html#method.binary_search_by
    fn binary_search_in_dir(
        &self,
        dir: &[(StringId, DirEntryId)],
        key: &str,
        strings: &StringInterner,
    ) -> Result<Result<usize, usize>, StorageError> {
        let mut error = None;

        let result = dir.binary_search_by(|value| {
            match strings.get_str(value.0) {
                Ok(value) => value.as_ref().cmp(key),
                Err(e) => {
                    // Take the error and stop the search
                    error = Some(e);
                    Ordering::Equal
                }
            }
        });

        if let Some(e) = error {
            return Err(e);
        };

        Ok(result)
    }

    fn dir_find_dir_entry_recursive(
        &self,
        inode_id: InodeId,
        key: &str,
        strings: &StringInterner,
    ) -> Option<DirEntryId> {
        let inode = self.get_inode(inode_id).ok()?;

        match inode {
            Inode::Directory(dir_id) => self.dir_find_dir_entry(*dir_id, key, strings),
            Inode::Pointers {
                depth, pointers, ..
            } => {
                let index_at_depth = index_of_key(*depth, key) as usize;

                let pointer = pointers.get(index_at_depth)?.as_ref()?;

                let inode_id = pointer.inode_id();
                self.dir_find_dir_entry_recursive(inode_id, key, strings)
            }
        }
    }

    /// Find `key` in the directory.
    pub fn dir_find_dir_entry(
        &self,
        dir_id: DirectoryId,
        key: &str,
        strings: &StringInterner,
    ) -> Option<DirEntryId> {
        if let Some(inode_id) = dir_id.get_inode_id() {
            self.dir_find_dir_entry_recursive(inode_id, key, strings)
        } else {
            let dir = self.get_small_dir(dir_id).ok()?;
            let index = self
                .binary_search_in_dir(dir.as_ref(), key, strings)
                .ok()?
                .ok()?;

            Some(dir[index].1)
        }
    }

    /// Move `new_dir` into `Self::directories` and return the `DirectoryId`.
    pub fn append_to_directories(
        &mut self,
        new_dir: &mut Vec<(StringId, DirEntryId)>,
    ) -> Result<DirectoryId, StorageError> {
        let (start, length) = self.directories.append(new_dir);

        DirectoryId::try_new_dir(start, start + length)
    }

    /// Use `self.temp_dir` to avoid allocations
    pub fn with_new_dir<F, R>(&mut self, fun: F) -> R
    where
        F: FnOnce(&mut Self, &mut Vec<(StringId, DirEntryId)>) -> R,
    {
        let mut new_dir = std::mem::take(&mut self.temp_dir);
        new_dir.clear();

        let result = fun(self, &mut new_dir);

        self.temp_dir = new_dir;
        result
    }

    pub fn add_inode(&mut self, inode: Inode) -> Result<InodeId, StorageError> {
        let current = self.inodes.push(inode)?;

        let current_index: usize = current.try_into().unwrap();
        if current_index & !FULL_31_BITS != 0 {
            // Must fit in 31 bits (See PointerToInode)
            return Err(StorageError::InodeIndexTooBig);
        }

        Ok(current)
    }

    fn sort_slice(
        slice: &mut [(StringId, DirEntryId)],
        strings: &StringInterner,
    ) -> Result<(), StorageError> {
        let mut error = None;

        slice.sort_unstable_by(|a, b| {
            let a = match strings.get_str(a.0) {
                Ok(a) => a,
                Err(e) => {
                    error = Some(e);
                    Cow::Borrowed("")
                }
            };

            let b = match strings.get_str(b.0) {
                Ok(b) => b,
                Err(e) => {
                    error = Some(e);
                    Cow::Borrowed("")
                }
            };

            a.cmp(&b)
        });

        if let Some(e) = error {
            return Err(e);
        };

        Ok(())
    }

    /// Copy directory from `Self::temp_dir` into `Self::directories` in a sorted order.
    ///
    /// `dir_range` is the range of the directory in `Self::temp_dir`
    /// The method first sorts `Self::temp_dir[dir_range]` in place, and copy the directory
    /// in `Self::directories`.
    fn copy_sorted(
        &mut self,
        dir_range: TempDirRange,
        strings: &StringInterner,
    ) -> Result<DirectoryId, StorageError> {
        let temp_dir = &mut self.temp_dir[dir_range];

        Self::sort_slice(temp_dir, strings)?;
        let (start, length) = self.directories.extend_from_slice(temp_dir);

        DirectoryId::try_new_dir(start, start + length)
    }

    fn with_temp_dir_range<Fun>(&mut self, mut fun: Fun) -> Result<TempDirRange, StorageError>
    where
        Fun: FnMut(&mut Self) -> Result<(), StorageError>,
    {
        let start = self.temp_dir.len();
        fun(self)?;
        let end = self.temp_dir.len();

        Ok(TempDirRange { start, end })
    }

    fn create_inode(
        &mut self,
        depth: u32,
        dir_range: TempDirRange,
        strings: &StringInterner,
    ) -> Result<InodeId, StorageError> {
        let dir_range_len = dir_range.end - dir_range.start;

        if dir_range_len <= INODE_POINTER_THRESHOLD {
            // The directory in `dir_range` is not guaranted to be sorted.
            // We use `Self::copy_sorted` to copy that directory into `Storage::directories` in
            // a sorted order.

            let new_dir_id = self.copy_sorted(dir_range, strings)?;

            self.add_inode(Inode::Directory(new_dir_id))
        } else {
            let nchildren = dir_range_len as u32;
            let mut pointers: [Option<PointerToInode>; 32] = Default::default();
            let mut npointers = 0;

            for index in 0..32u8 {
                let range = self.with_temp_dir_range(|this| {
                    for i in dir_range.clone() {
                        let (key_id, dir_entry_id) = this.temp_dir[i];
                        let key = strings.get_str(key_id)?;
                        if index_of_key(depth, &key) as u8 == index {
                            this.temp_dir.push((key_id, dir_entry_id));
                        }
                    }
                    Ok(())
                })?;

                if range.is_empty() {
                    continue;
                }

                npointers += 1;
                let inode_id = self.create_inode(depth + 1, range, strings)?;

                pointers[index as usize] = Some(PointerToInode::new(None, inode_id));
            }

            self.add_inode(Inode::Pointers {
                depth,
                nchildren,
                npointers,
                pointers,
            })
        }
    }

    /// Insert `(key_id, dir_entry)` into `Self::temp_dir`.
    fn insert_dir_single_dir_entry(
        &mut self,
        key_id: StringId,
        dir_entry: DirEntry,
    ) -> Result<TempDirRange, StorageError> {
        let dir_entry_id = self.nodes.push(dir_entry)?;

        self.with_temp_dir_range(|this| {
            this.temp_dir.push((key_id, dir_entry_id));
            Ok(())
        })
    }

    /// Copy directory `dir_id` from `Self::directories` into `Self::temp_dir`
    ///
    /// Note: The callers of this function expect the directory to be copied
    ///       at the end of `Self::temp_dir`. This is something to keep in mind
    ///       if the implementation of this function change
    fn copy_dir_in_temp_dir(&mut self, dir_id: DirectoryId) -> Result<TempDirRange, StorageError> {
        let (dir_start, dir_end) = dir_id.get();

        self.with_temp_dir_range(|this| {
            let dir = this
                .directories
                .get_slice(dir_start..dir_end)
                .ok_or(StorageError::DirNotFound)?;

            this.temp_dir.extend_from_slice(dir.as_ref());
            Ok(())
        })
    }

    fn insert_inode(
        &mut self,
        depth: u32,
        inode_id: InodeId,
        key: &str,
        key_id: StringId,
        dir_entry: DirEntry,
        strings: &StringInterner,
    ) -> Result<(InodeId, IsNewKey), StorageError> {
        let inode = self.get_inode(inode_id)?;

        match inode {
            Inode::Directory(dir_id) => {
                let dir_id = *dir_id;
                let dir_entry_id = self.add_dir_entry(dir_entry)?;

                // Copy the existing directory into `Self::temp_dir` to create an inode
                let range = self.with_temp_dir_range(|this| {
                    let range = this.copy_dir_in_temp_dir(dir_id)?;

                    // We're using `Vec::insert` below and we don't want to invalidate
                    // any existing `TempDirRange`
                    debug_assert_eq!(range.end, this.temp_dir.len());

                    let start = range.start;
                    match this.binary_search_in_dir(&this.temp_dir[range], key, strings)? {
                        Ok(found) => this.temp_dir[start + found] = (key_id, dir_entry_id),
                        Err(index) => this.temp_dir.insert(start + index, (key_id, dir_entry_id)),
                    }

                    Ok(())
                })?;

                let new_inode_id = self.create_inode(depth, range, strings)?;
                let is_new_key = self.inode_len(new_inode_id)? != dir_id.small_dir_len();

                Ok((new_inode_id, is_new_key))
            }
            Inode::Pointers {
                depth,
                nchildren,
                mut npointers,
                pointers,
            } => {
                let mut pointers = pointers.clone();
                let nchildren = *nchildren;
                let depth = *depth;

                let index_at_depth = index_of_key(depth, key) as usize;

                let (inode_id, is_new_key) = if let Some(pointer) = &pointers[index_at_depth] {
                    let inode_id = pointer.inode_id();
                    self.insert_inode(depth + 1, inode_id, key, key_id, dir_entry, strings)?
                } else {
                    npointers += 1;

                    let new_dir_id = self.insert_dir_single_dir_entry(key_id, dir_entry)?;
                    let inode_id = self.create_inode(depth, new_dir_id, strings)?;
                    (inode_id, true)
                };

                pointers[index_at_depth] = Some(PointerToInode::new(None, inode_id));

                let inode_id = self.add_inode(Inode::Pointers {
                    depth,
                    nchildren: if is_new_key { nchildren + 1 } else { nchildren },
                    npointers,
                    pointers,
                })?;

                Ok((inode_id, is_new_key))
            }
        }
    }

    /// [test only] Remove hash ids in the inode and it's children
    ///
    /// This is used to force recomputing hashes
    #[cfg(test)]
    pub fn inodes_drop_hash_ids(&self, inode_id: InodeId) {
        let inode = self.get_inode(inode_id).unwrap();

        if let Inode::Pointers { pointers, .. } = inode {
            for pointer in pointers.iter().filter_map(|p| p.as_ref()) {
                pointer.set_hash_id(None);

                let inode_id = pointer.inode_id();
                self.inodes_drop_hash_ids(inode_id);
            }
        };
    }

    fn iter_inodes_recursive_unsorted<Fun>(
        &self,
        inode: &Inode,
        fun: &mut Fun,
    ) -> Result<(), MerkleError>
    where
        Fun: FnMut(&(StringId, DirEntryId)) -> Result<(), MerkleError>,
    {
        match inode {
            Inode::Pointers { pointers, .. } => {
                for pointer in pointers.iter().filter_map(|p| p.as_ref()) {
                    let inode_id = pointer.inode_id();
                    let inode = self.get_inode(inode_id)?;
                    self.iter_inodes_recursive_unsorted(inode, fun)?;
                }
            }
            Inode::Directory(dir_id) => {
                let dir = self.get_small_dir(*dir_id)?;
                for elem in dir.as_ref() {
                    fun(elem)?;
                }
            }
        };

        Ok(())
    }

    /// Iterate on `dir_id`.
    ///
    /// The elements won't be sorted when the underlying directory is an `Inode`.
    /// `Self::dir_to_vec_sorted` can be used to get the directory sorted.
    pub fn dir_iterate_unsorted<Fun>(
        &self,
        dir_id: DirectoryId,
        mut fun: Fun,
    ) -> Result<(), MerkleError>
    where
        Fun: FnMut(&(StringId, DirEntryId)) -> Result<(), MerkleError>,
    {
        if let Some(inode_id) = dir_id.get_inode_id() {
            let inode = self.get_inode(inode_id)?;

            self.iter_inodes_recursive_unsorted(inode, &mut fun)?;
        } else {
            let dir = self.get_small_dir(dir_id)?;
            for elem in dir.as_ref() {
                fun(elem)?;
            }
        }
        Ok(())
    }

    fn inode_len(&self, inode_id: InodeId) -> Result<usize, StorageError> {
        let inode = self.get_inode(inode_id)?;
        match inode {
            Inode::Pointers {
                nchildren: children,
                ..
            } => Ok(*children as usize),
            Inode::Directory(dir_id) => Ok(dir_id.small_dir_len()),
        }
    }

    /// Return the number of nodes in `dir_id`.
    pub fn dir_len(&self, dir_id: DirectoryId) -> Result<usize, StorageError> {
        if let Some(inode_id) = dir_id.get_inode_id() {
            self.inode_len(inode_id)
        } else {
            Ok(dir_id.small_dir_len())
        }
    }

    /// Make a vector of `(StringId, DirEntryId)`
    ///
    /// The vector won't be sorted when the underlying directory is an Inode.
    /// `Self::dir_to_vec_sorted` can be used to get the vector sorted.
    pub fn dir_to_vec_unsorted(
        &self,
        dir_id: DirectoryId,
    ) -> Result<Vec<(StringId, DirEntryId)>, MerkleError> {
        let mut vec = Vec::with_capacity(self.dir_len(dir_id)?);

        self.dir_iterate_unsorted(dir_id, |&(key_id, dir_entry_id)| {
            vec.push((key_id, dir_entry_id));
            Ok(())
        })?;

        Ok(vec)
    }

    /// Make a vector of `(StringId, DirEntryId)` sorted by the key
    ///
    /// This is an expensive method when the underlying directory is an Inode,
    /// `Self::dir_to_vec_unsorted` should be used when the ordering is
    /// not important
    pub fn dir_to_vec_sorted(
        &self,
        dir_id: DirectoryId,
        strings: &StringInterner,
    ) -> Result<Vec<(StringId, DirEntryId)>, MerkleError> {
        if dir_id.get_inode_id().is_some() {
            let mut dir = self.dir_to_vec_unsorted(dir_id)?;
            Self::sort_slice(&mut dir, strings)?;

            Ok(dir)
        } else {
            let dir = self.get_small_dir(dir_id)?;
            Ok(dir.to_vec())
        }
    }

    pub fn get_inode(&self, inode_id: InodeId) -> Result<&Inode, StorageError> {
        self.inodes
            .get(inode_id)?
            .ok_or(StorageError::InodeNotFound)
    }

    /// Inserts the key into the directory.
    ///
    /// Returns the newly created directory `DirectoryId`.
    /// If the key already exists in this directory, this replace the dir_entry.
    pub fn dir_insert(
        &mut self,
        dir_id: DirectoryId,
        key_str: &str,
        dir_entry: DirEntry,
        strings: &mut StringInterner,
    ) -> Result<DirectoryId, StorageError> {
        let key_id = strings.make_string_id(key_str);

        // Are we inserting in an Inode ?
        if let Some(inode_id) = dir_id.get_inode_id() {
            let (inode_id, _) =
                self.insert_inode(0, inode_id, key_str, key_id, dir_entry, strings)?;
            self.temp_dir.clear();
            return Ok(inode_id.into());
        }

        let dir_entry_id = self.nodes.push(dir_entry)?;

        let dir_id = self.with_new_dir(|this, new_dir| {
            let dir = this.get_small_dir(dir_id)?;

            let index = this.binary_search_in_dir(dir.as_ref(), key_str, strings)?;

            match index {
                Ok(found) => {
                    new_dir.extend_from_slice(dir.as_ref());
                    new_dir[found].1 = dir_entry_id;
                }
                Err(index) => {
                    new_dir.extend_from_slice(&dir[..index]);
                    new_dir.push((key_id, dir_entry_id));
                    new_dir.extend_from_slice(&dir[index..]);
                }
            }

            this.append_to_directories(new_dir)
        })?;

        // We only check at the end of this function if the new directory length
        // is > DIRECTORY_INODE_THRESHOLD because inserting an element in a directory of length
        // DIRECTORY_INODE_THRESHOLD doesn't necessary mean that the resulting directory will
        // be bigger (if the key already exist).
        let dir_len = dir_id.small_dir_len();

        if dir_len <= DIRECTORY_INODE_THRESHOLD {
            Ok(dir_id)
        } else {
            // Copy the new directory in `Self::temp_dir`.
            let range = self.copy_dir_in_temp_dir(dir_id)?;
            // Remove the newly created directory from `Self::directories` to save memory.
            // It won't be used anymore as we're creating an inode.
            self.directories.remove_last_nelems(dir_len);

            let inode_id = self.create_inode(0, range, strings)?;
            self.temp_dir.clear();

            Ok(inode_id.into())
        }
    }

    fn remove_in_inode_recursive(
        &mut self,
        inode_id: InodeId,
        key: &str,
        strings: &StringInterner,
    ) -> Result<Option<InodeId>, StorageError> {
        let inode = self.get_inode(inode_id)?;

        match inode {
            Inode::Directory(dir_id) => {
                let dir_id = *dir_id;
                let new_dir_id = self.dir_remove(dir_id, key, strings)?;

                if new_dir_id.is_empty() {
                    // The directory is now empty, return None to indicate that it
                    // should be removed from the Inode::Pointers
                    Ok(None)
                } else if new_dir_id == dir_id {
                    // The key was not found in the directory, so it's the same directory.
                    // Do not create a new inode.
                    Ok(Some(inode_id))
                } else {
                    self.add_inode(Inode::Directory(new_dir_id)).map(Some)
                }
            }
            Inode::Pointers {
                depth,
                nchildren,
                npointers,
                pointers,
            } => {
                let depth = *depth;
                let mut npointers = *npointers;
                let nchildren = *nchildren;
                let new_nchildren = nchildren - 1;

                let new_inode_id = if new_nchildren as usize <= INODE_POINTER_THRESHOLD {
                    // After removing an element from this `Inode::Pointers`, it remains
                    // INODE_POINTER_THRESHOLD items, so it should be converted to a
                    // `Inode::Directory`.

                    let dir_id = self.inodes_to_dir_sorted(inode_id, strings)?;
                    let new_dir_id = self.dir_remove(dir_id, key, strings)?;

                    if dir_id == new_dir_id {
                        // The key was not found.

                        // Remove the directory that was just created with
                        // Self::inodes_to_dir_sorted above, it won't be used and save space.
                        self.directories.remove_last_nelems(dir_id.small_dir_len());
                        return Ok(Some(inode_id));
                    }

                    self.add_inode(Inode::Directory(new_dir_id))?
                } else {
                    let index_at_depth = index_of_key(depth, key) as usize;

                    let pointer = match pointers[index_at_depth].as_ref() {
                        Some(pointer) => pointer,
                        None => return Ok(Some(inode_id)), // The key was not found
                    };

                    let ptr_inode_id = pointer.inode_id();
                    let mut pointers = pointers.clone();

                    match self.remove_in_inode_recursive(ptr_inode_id, key, strings)? {
                        Some(new_ptr_inode_id) if new_ptr_inode_id == ptr_inode_id => {
                            // The key was not found, don't create a new inode
                            return Ok(Some(inode_id));
                        }
                        Some(new_ptr_inode_id) => {
                            pointers[index_at_depth] =
                                Some(PointerToInode::new(None, new_ptr_inode_id));
                        }
                        None => {
                            // The key was removed and it result in an empty directory.
                            // Remove the pointer: make it `None`.
                            pointers[index_at_depth] = None;
                            npointers -= 1;
                        }
                    }

                    self.add_inode(Inode::Pointers {
                        depth,
                        nchildren: new_nchildren,
                        npointers,
                        pointers,
                    })?
                };

                Ok(Some(new_inode_id))
            }
        }
    }

    /// Convert the Inode into a small directory
    ///
    /// This traverses all elements (all children) of this `inode_id` and
    /// copy them into `Self::directories` in a sorted order.
    fn inodes_to_dir_sorted(
        &mut self,
        inode_id: InodeId,
        strings: &StringInterner,
    ) -> Result<DirectoryId, StorageError> {
        let start = self.temp_dir.len();

        // Iterator on the inodes children and copy all nodes into `Self::temp_dir`
        self.with_new_dir::<_, Result<_, StorageError>>(|this, temp_dir| {
            let inode = this.get_inode(inode_id)?;

            this.iter_inodes_recursive_unsorted(inode, &mut |value| {
                temp_dir.push(*value);
                Ok(())
            })
            .map_err(|_| StorageError::IterationError)?;

            Ok(())
        })?;

        // Copy nodes from `Self::temp_dir` into `Self::directories` sorted
        self.copy_sorted(
            TempDirRange {
                start,
                end: self.temp_dir.len(),
            },
            strings,
        )
    }

    fn remove_in_inode(
        &mut self,
        inode_id: InodeId,
        key: &str,
        strings: &StringInterner,
    ) -> Result<DirectoryId, StorageError> {
        let inode_id = self.remove_in_inode_recursive(inode_id, key, strings)?;
        let inode_id = inode_id.ok_or(StorageError::RootOfInodeNotAPointer)?;

        if self.inode_len(inode_id)? > DIRECTORY_INODE_THRESHOLD {
            Ok(inode_id.into())
        } else {
            // There is now DIRECTORY_INODE_THRESHOLD or less items:
            // Convert the inode into a 'small' directory
            self.inodes_to_dir_sorted(inode_id, strings)
        }
    }

    /// Remove `key` from the directory and return the new `DirectoryId`.
    ///
    /// If the key doesn't exist, it returns the same `DirectoryId`.
    pub fn dir_remove(
        &mut self,
        dir_id: DirectoryId,
        key: &str,
        strings: &StringInterner,
    ) -> Result<DirectoryId, StorageError> {
        if let Some(inode_id) = dir_id.get_inode_id() {
            return self.remove_in_inode(inode_id, key, strings);
        };

        self.with_new_dir(|this, new_dir| {
            let dir = this.get_small_dir(dir_id)?;

            if dir.is_empty() {
                return Ok(dir_id);
            }

            let index = match this.binary_search_in_dir(dir.as_ref(), key, strings)? {
                Ok(index) => index,
                Err(_) => return Ok(dir_id), // The key was not found
            };

            if index > 0 {
                new_dir.extend_from_slice(&dir[..index]);
            }
            if index + 1 != dir.len() {
                new_dir.extend_from_slice(&dir[index + 1..]);
            }

            this.append_to_directories(new_dir)
        })
    }

    pub fn clear(&mut self) {
        if self.blobs.capacity() > DEFAULT_BLOBS_CAPACITY {
            self.blobs = ChunkedVec::with_chunk_capacity(DEFAULT_BLOBS_CAPACITY);
        } else {
            self.blobs.clear();
        }

        if self.nodes.capacity() > DEFAULT_NODES_CAPACITY {
            self.nodes = IndexMap::with_chunk_capacity(DEFAULT_NODES_CAPACITY);
        } else {
            self.nodes.clear();
        }

        if self.directories.capacity() > DEFAULT_DIRECTORIES_CAPACITY {
            self.directories = ChunkedVec::with_chunk_capacity(DEFAULT_DIRECTORIES_CAPACITY);
        } else {
            self.directories.clear();
        }

        if self.inodes.capacity() > DEFAULT_INODES_CAPACITY {
            self.inodes = IndexMap::with_chunk_capacity(DEFAULT_INODES_CAPACITY);
        } else {
            self.inodes.clear();
        }
    }

    pub fn deallocate(&mut self) {
        self.nodes = IndexMap::empty();
        self.directories = ChunkedVec::empty();
        self.temp_dir = Vec::new();
        self.blobs = ChunkedVec::empty();
        self.inodes = IndexMap::empty();
        self.data = Vec::new();
        self.offsets_to_hash_id = HashMap::default();
    }
}

#[cfg(test)]
mod tests {
    use crate::working_tree::{DirEntryKind::Blob, Object};

    use super::*;

    #[test]
    fn test_storage() {
        let mut storage = Storage::new();
        let mut strings = StringInterner::default();

        let blob_id = storage.add_blob_by_ref(&[1]).unwrap();
        let object = Object::Blob(blob_id);

        let blob2_id = storage.add_blob_by_ref(&[2]).unwrap();
        let object2 = Object::Blob(blob2_id);

        let dir_entry1 = DirEntry::new(Blob, object.clone());
        let dir_entry2 = DirEntry::new(Blob, object2.clone());

        let dir_id = DirectoryId::empty();
        let dir_id = storage
            .dir_insert(dir_id, "a", dir_entry1.clone(), &mut strings)
            .unwrap();
        let dir_id = storage
            .dir_insert(dir_id, "b", dir_entry2.clone(), &mut strings)
            .unwrap();
        let dir_id = storage
            .dir_insert(dir_id, "0", dir_entry1.clone(), &mut strings)
            .unwrap();

        assert_eq!(
            storage.get_owned_dir(dir_id, &strings).unwrap(),
            &[
                ("0".to_string(), dir_entry1.clone()),
                ("a".to_string(), dir_entry1.clone()),
                ("b".to_string(), dir_entry2.clone()),
            ]
        );
    }

    #[test]
    fn test_blob_id() {
        let mut storage = Storage::new();

        let slice1 = &[0xFF, 0xFF, 0xFF];
        let slice2 = &[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
        let slice3 = &[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
        let slice4 = &[];

        let blob1 = storage.add_blob_by_ref(slice1).unwrap();
        let blob2 = storage.add_blob_by_ref(slice2).unwrap();
        let blob3 = storage.add_blob_by_ref(slice3).unwrap();
        let blob4 = storage.add_blob_by_ref(slice4).unwrap();

        assert!(blob1.is_inline());
        assert!(!blob2.is_inline());
        assert!(blob3.is_inline());
        assert!(!blob4.is_inline());

        assert_eq!(storage.get_blob(blob1).unwrap().as_ref(), slice1);
        assert_eq!(storage.get_blob(blob2).unwrap().as_ref(), slice2);
        assert_eq!(storage.get_blob(blob3).unwrap().as_ref(), slice3);
        assert_eq!(storage.get_blob(blob4).unwrap().as_ref(), slice4);
    }
}
