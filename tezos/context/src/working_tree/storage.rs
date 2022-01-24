// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! The "storage" is where all data used in the working tree is allocated and stored.
//! Data is represented in a flat form and kept in a contiguous memory area, which is more compact
//! and avoids memory fragmentation. Instead of pointers, special IDs encoding extra information
//! are used as references to different types of values.

use std::{
    borrow::Cow,
    cell::{Cell, RefCell},
    cmp::Ordering,
    collections::{HashMap, HashSet},
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
    persistent::file::FileOffset,
    working_tree::ObjectReference,
    ContextKeyValueStore, Map, ObjectHash,
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

#[derive(Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
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

impl std::fmt::Debug for DirectoryId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_inode() {
            f.debug_struct("DirectoryId")
                .field("InodeId", &self.get_inode_id())
                .finish()
        } else {
            let (start, end) = self.get();
            let length = end - start;
            f.debug_struct("DirectoryId")
                .field("start", &start)
                .field("end", &end)
                .field("length", &length)
                .finish()
        }
    }
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
    #[error("InodeNotFound: Inode pointers has not been found")]
    InodePointersNotFound,
    #[error("ExpectedDirGotInode: Expected a Dir but got an Inode")]
    ExpectedDirGotInode,
    #[error("IterationError: Iteration on an Inode failed")]
    IterationError,
    #[error("RootOfInodeNotAPointer: The root of an Inode must be a pointer")]
    RootOfInodeNotAPointer,
    #[error("InodeInRepositoryNotFound: Inode cannot be found in repository")]
    InodeInRepositoryNotFound,
    #[error("InodePointerIdNotFound: Inode does not have a PointerId")]
    InodePointerIdNotFound,
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

#[derive(Clone, Debug, Copy, PartialEq, Eq)]
pub struct PointerId(u32);

impl TryInto<usize> for PointerId {
    type Error = DirEntryIdError;

    fn try_into(self) -> Result<usize, Self::Error> {
        Ok(self.0 as usize)
    }
}

impl From<PointerId> for u32 {
    fn from(val: PointerId) -> Self {
        val.0
    }
}

impl From<u32> for PointerId {
    fn from(v: u32) -> Self {
        Self(v)
    }
}

impl TryFrom<usize> for PointerId {
    type Error = DirEntryIdError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        value.try_into().map(PointerId).map_err(|_| DirEntryIdError)
    }
}

impl From<u32> for InodeId {
    fn from(v: u32) -> Self {
        Self(v)
    }
}

impl From<InodeId> for u32 {
    fn from(v: InodeId) -> Self {
        v.0
    }
}

/// Describes which pointers are set and at what index.
///
/// `Inode::Pointers` is an array of 32 pointers.
/// Each pointer is either set (`Some`) or not set (`None`).
///
/// Example:
/// Let's say that there are 2 pointers sets in the array, at the index
/// 1 and 7.
/// This would be represented in this bitfield as:
/// `0b00000000_00000000_00000000_10000010`
///
#[derive(Copy, Clone, Default, Debug)]
pub struct PointersBitfield {
    bitfield: u32,
}

impl PointersBitfield {
    /// Set bit at index in the bitfield
    fn set(&mut self, index: usize) {
        self.bitfield |= 1 << index;
    }

    /// Get bit at index in the bitfield
    fn get(&self, index: usize) -> bool {
        self.bitfield & 1 << index != 0
    }

    fn get_index_for(&self, index: usize) -> Option<usize> {
        if !self.get(index) {
            return None;
        }

        let index = index as u32;

        let bitfield = self.bitfield.checked_shl(32 - index).unwrap_or(0);

        let index = bitfield.count_ones() as usize;

        Some(index)
    }

    pub fn to_bytes(self) -> [u8; 4] {
        self.bitfield.to_le_bytes()
    }

    /// Iterates on all the bit sets in the bitfield.
    ///
    /// The iterator returns the index of the bit.
    pub fn iter(&self) -> PointersBitfieldIterator {
        PointersBitfieldIterator {
            bitfield: *self,
            current: 0,
        }
    }

    pub fn from_bytes(bytes: [u8; 4]) -> Self {
        Self {
            bitfield: u32::from_le_bytes(bytes),
        }
    }

    /// Count number of bit set in the bitfield.
    pub fn count(&self) -> u8 {
        self.bitfield.count_ones() as u8
    }
}

impl From<&[Option<PointerOnStack>; 32]> for PointersBitfield {
    fn from(pointers: &[Option<PointerOnStack>; 32]) -> Self {
        let mut bitfield = Self::default();

        for (index, pointer) in pointers.iter().enumerate() {
            if pointer.is_some() {
                bitfield.set(index);
            }
        }

        bitfield
    }
}

/// Iterates on all the bit sets in the bitfield.
///
/// The iterator returns the index of the bit.
pub struct PointersBitfieldIterator {
    bitfield: PointersBitfield,
    current: usize,
}

impl Iterator for PointersBitfieldIterator {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        for index in self.current..32 {
            if self.bitfield.get(index) {
                self.current = index + 1;
                return Some(index);
            }
        }

        None
    }
}

// #[bitfield]
// #[derive(Clone, Debug, Copy, PartialEq, Eq)]
#[derive(Debug, Clone, Copy)]
pub struct PointersId {
    start: u32,
    bitfield: PointersBitfield,
}

impl From<(u32, &[Option<PointerOnStack>; 32])> for PointersId {
    fn from((start, pointers): (u32, &[Option<PointerOnStack>; 32])) -> Self {
        let bitfield = PointersBitfield::from(pointers);
        Self { start, bitfield }
    }
}

impl PointersId {
    fn get_start(&self) -> usize {
        self.start as usize
    }

    pub fn npointers(&self) -> usize {
        self.bitfield.count() as usize
    }

    pub fn bitfield(&self) -> PointersBitfield {
        self.bitfield
    }

    fn get_index_for_ptr(&self, ptr_index: usize) -> Option<usize> {
        let index = self.start as usize;
        let offset = self.bitfield.get_index_for(ptr_index)?;

        Some(index + offset)
    }

    pub fn iter(self) -> InodePointersIter {
        InodePointersIter::new(self)
    }
}

// #[bitfield]
// #[derive(Clone, Copy, Debug)]
// pub struct PointerToInodeInner {
//     hash_id: B48,
//     is_commited: bool,
//     is_inode_available: bool,
//     inode_id: B31,
//     /// Set to `0` when the offset is not set
//     offset: B63,
// }

#[derive(BitfieldSpecifier)]
#[bits = 2]
#[derive(Debug, Copy, Clone)]
enum PointerPtrKind {
    Directory,
    HashId,
    Offset,
}

#[derive(BitfieldSpecifier)]
#[bits = 1]
#[derive(Debug, Copy, Clone)]
enum ThinPointerKind {
    InodeId,
    BigPointer,
}

enum ThinPointerValue {
    Inode(InodeId),
    BigPointer(PointerId),
}

#[bitfield]
#[derive(Debug, Clone)]
pub struct ThinPointer {
    is_commited: bool,
    ref_kind: ThinPointerKind,
    /// Depending on `ref_kind`, this is either a `InodeId` or a `BigPointerId`
    value: B30,
}

assert_eq_size!([u8; 4], ThinPointer);

impl ThinPointer {
    fn get_value(&self) -> ThinPointerValue {
        match self.ref_kind() {
            ThinPointerKind::InodeId => ThinPointerValue::Inode(self.value().into()),
            ThinPointerKind::BigPointer => ThinPointerValue::BigPointer(self.value().into()),
        }
    }
}

#[bitfield]
#[derive(Clone, Copy, Debug)]
pub struct PointerInner {
    is_commited: bool,
    ptr_kind: PointerPtrKind,
    /// This is either a:
    /// - `DirectoryId`
    /// - `InodeId`
    /// - `HashId`
    /// - `AbsoluteOffset`
    ptr_id: B61,
}

#[derive(Clone, Debug)]
pub struct FatPointer {
    inner: Cell<PointerInner>,
}

assert_eq_size!([u8; 8], FatPointer);

pub struct PointerOnStack {
    pub thin_pointer: Option<ThinPointer>,
    pub fat_pointer: FatPointer,
}

impl std::ops::Deref for PointerOnStack {
    type Target = FatPointer;

    fn deref(&self) -> &Self::Target {
        &self.fat_pointer
    }
}

impl FatPointer {
    pub fn new(dir_or_inode_id: DirectoryOrInodeId) -> Self {
        Self {
            inner: Cell::new(
                PointerInner::new()
                    .with_is_commited(false)
                    .with_ptr_id(dir_or_inode_id.as_u64())
                    .with_ptr_kind(PointerPtrKind::Directory),
            ),
        }
    }

    pub fn new_commited(hash_id: Option<HashId>, offset: Option<AbsoluteOffset>) -> Self {
        let (ptr_kind, ptr) = match (hash_id, offset) {
            (None, Some(offset)) => (PointerPtrKind::Offset, offset.as_u64()),
            (Some(hash_id), None) => (PointerPtrKind::HashId, hash_id.as_u64()),
            _ => unreachable!(
                "Self::new_commited must be call with a `HashId` or an `AbsoluteOffset`"
            ),
        };

        Self {
            inner: Cell::new(
                PointerInner::new()
                    .with_is_commited(true)
                    .with_ptr_kind(ptr_kind)
                    .with_ptr_id(ptr),
            ),
        }
    }

    fn from_thin_pointer(
        thin_pointer: ThinPointer,
        storage: &Storage,
    ) -> Result<Self, StorageError> {
        let value: u32 = thin_pointer.value();

        match thin_pointer.ref_kind() {
            ThinPointerKind::InodeId => {
                let inode_id: InodeId = value.into();
                let inode_id = DirectoryId::from(inode_id);
                let inode_id: u64 = inode_id.into();

                let is_commited: bool = thin_pointer.is_commited();
                Ok(Self {
                    inner: Cell::new(
                        PointerInner::new()
                            .with_is_commited(is_commited)
                            .with_ptr_kind(PointerPtrKind::Directory)
                            .with_ptr_id(inode_id),
                    ),
                })
            }
            ThinPointerKind::BigPointer => {
                let pointer_id: PointerId = (value as usize).try_into()?;
                let fat_pointer = storage
                    .fat_pointers
                    .get(pointer_id)?
                    .cloned()
                    .ok_or(StorageError::InodePointersNotFound)?;

                Ok(fat_pointer)
            }
        }
    }

    pub fn ptr_id(&self) -> Option<DirectoryOrInodeId> {
        let inner = self.inner.get();

        if !matches!(inner.ptr_kind(), PointerPtrKind::Directory) {
            return None;
        }

        let ptr_id: u64 = inner.ptr_id();
        let dir_id = DirectoryId::from(ptr_id);

        if let Some(inode_id) = dir_id.get_inode_id() {
            Some(DirectoryOrInodeId::Inode(inode_id))
        } else {
            Some(DirectoryOrInodeId::Directory(dir_id))
        }
    }

    pub fn get_data(&self) -> Option<ObjectReference> {
        let inner = self.inner.get();

        let ptr_id: u64 = inner.ptr_id();

        match inner.ptr_kind() {
            PointerPtrKind::Directory => None,
            PointerPtrKind::HashId => {
                let hash_id = HashId::new(ptr_id).unwrap();
                Some(ObjectReference::new(Some(hash_id), None))
            }
            PointerPtrKind::Offset => {
                let offset: AbsoluteOffset = ptr_id.into();
                Some(ObjectReference::new(None, Some(offset)))
            }
        }
    }

    pub fn set_ptr_id(&self, ptr_id: DirectoryOrInodeId) {
        let mut inner = self.inner.get();

        inner.set_ptr_id(ptr_id.as_u64());
        inner.set_ptr_kind(PointerPtrKind::Directory);

        self.inner.set(inner);
    }

    pub fn is_commited(&self) -> bool {
        let inner = self.inner.get();
        inner.is_commited()
    }

    pub fn set_commited(&self, value: bool) {
        let mut inner = self.inner.get();
        inner.set_is_commited(value);
        self.inner.set(inner);
    }
}

pub struct InodePointersIter {
    start: usize,
    bitfield_iter: PointersBitfieldIterator,
    current: usize,
}

impl InodePointersIter {
    fn new(pointers: PointersId) -> Self {
        Self {
            start: pointers.get_start(),
            bitfield_iter: pointers.bitfield.iter(),
            current: 0,
        }
    }
}

impl Iterator for InodePointersIter {
    type Item = (usize, usize);

    fn next(&mut self) -> Option<Self::Item> {
        let next_ptr_index = self.bitfield_iter.next()?;
        let current = self.current;
        self.current += 1;

        Some((next_ptr_index, self.start + current))
    }
}

#[derive(Debug)]
pub struct Inode {
    pub depth: u16,
    pub nchildren: u32,
    pub pointers: PointersId,
}

assert_eq_size!([u8; 16], Inode);

/// This enum is not stored in `Storage`
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum DirectoryOrInodeId {
    Directory(DirectoryId),
    Inode(InodeId),
}

impl DirectoryOrInodeId {
    pub fn as_u64(&self) -> u64 {
        let dir_id: DirectoryId = match self {
            DirectoryOrInodeId::Directory(dir_id) => *dir_id,
            DirectoryOrInodeId::Inode(inode_id) => (*inode_id).into(),
        };

        dir_id.into()
    }

    pub fn into_dir(self) -> DirectoryId {
        match self {
            DirectoryOrInodeId::Directory(dir_id) => dir_id,
            DirectoryOrInodeId::Inode(inode_id) => inode_id.into(),
        }
    }
}

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
    /// Temporary directories, this is used to avoid allocations when we
    /// manipulate `directories`
    /// For example, `Storage::insert` will create a new directory in `temp_dir`, once
    /// done it will copy that directory from `temp_dir` into the end of `directories`
    temp_dir: Vec<(StringId, DirEntryId)>,
    /// Vector where we temporary store the hashes (`hash::index_of_key()`) of
    /// `Self::temp_dir`.
    /// The hash of `Self::temp_dir[X]` is at `Self::temp_inodes_index[X]`
    temp_inodes_index: Vec<u8>,
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

    thin_pointers: ChunkedVec<ThinPointer>,
    fat_pointers: IndexMap<PointerId, FatPointer>,
    pointers_data: RefCell<HashMap<u64, ObjectReference>>,
    /// Objects bytes are read from disk into this vector
    pub data: Vec<u8>,
    /// Map of deserialized (from disk) offset to their `HashId`.
    pub offsets_to_hash_id: Map<AbsoluteOffset, HashId>,
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

const DEFAULT_DIRECTORIES_CAPACITY: usize = 512 * 1024;
const DEFAULT_BLOBS_CAPACITY: usize = 128 * 1024;
const DEFAULT_NODES_CAPACITY: usize = 128 * 1024;
const DEFAULT_INODES_CAPACITY: usize = 32 * 1024;

impl Storage {
    pub fn new() -> Self {
        Self {
            directories: ChunkedVec::with_chunk_capacity(DEFAULT_DIRECTORIES_CAPACITY), // ~4MB
            temp_dir: Vec::with_capacity(256),                                          // 2KB
            // Allocates `temp_inodes_index` only when used
            temp_inodes_index: Vec::new(), // 0B
            blobs: ChunkedVec::with_chunk_capacity(DEFAULT_BLOBS_CAPACITY), // 128KB
            nodes: IndexMap::with_chunk_capacity(DEFAULT_NODES_CAPACITY), // ~3MB
            inodes: IndexMap::with_chunk_capacity(DEFAULT_INODES_CAPACITY), // ~20MB
            data: Vec::with_capacity(100_000), // ~97KB
            offsets_to_hash_id: Map::default(),
            fat_pointers: IndexMap::with_chunk_capacity(32 * 1024),
            pointers_data: Default::default(),
            thin_pointers: ChunkedVec::with_chunk_capacity(32 * 1024),
            // pointers_offsets: Default::default(),
            // pointers_hash_id: Default::default(),
        } // Total ~27MB
    }

    pub fn memory_usage(&self, strings: &StringInterner) -> StorageMemoryUsage {
        // let mut npointers = 0;
        // let mut ndir = 0;

        // for inode in self.inodes.iter_values() {
        //     let mut only_pointers = true;
        //     for (_, index) in self.iter_pointers_with_index(inode.pointers) {
        //         let pointer = self.pointers.get(index).unwrap();

        //         if let Some(ptr_id) = pointer.ptr_id() {
        //             match ptr_id {
        //                 DirectoryOrInodeId::Directory(_) => only_pointers = false,
        //                 DirectoryOrInodeId::Inode(_) => {},
        //             }
        //         } else {
        //             only_pointers = false;
        //         }

        //         if only_pointers {
        //             npointers += 1;
        //         }
        //     }
        // }

        // let mut unique_dirs = HashSet::<DirectoryId>::default();

        // for pointer in self.pointers.iter() {
        //     if let Some(ptr_id) = pointer.ptr_id() {
        //         match ptr_id {
        //             DirectoryOrInodeId::Directory(dir_id) => {
        //                 unique_dirs.insert(dir_id);
        //                 ndir += 1
        //             }
        //             DirectoryOrInodeId::Inode(_) => npointers += 1,
        //         }
        //     };
        // }

        // println!(
        //     "NDIR={:?} NPOINTERS={:?} UNIQUE_DIR={:?}",
        //     ndir,
        //     npointers,
        //     unique_dirs.len()
        // );

        let nodes_cap = self.nodes.capacity();
        let directories_cap = self.directories.capacity();
        let blobs_cap = self.blobs.capacity();
        let temp_dir_cap = self.temp_dir.capacity();
        let temp_inodes_index_cap = self.temp_inodes_index.capacity();
        let inodes_cap = self.inodes.capacity();
        let pointers_refs_cap = self.thin_pointers.capacity();
        let pointers_cap = self.fat_pointers.capacity();
        let strings = strings.memory_usage();
        let total_bytes = (nodes_cap * size_of::<DirEntry>())
            .saturating_add(directories_cap * size_of::<(StringId, DirEntryId)>())
            .saturating_add(temp_dir_cap * size_of::<(StringId, DirEntryId)>())
            .saturating_add(temp_inodes_index_cap * size_of::<u8>())
            .saturating_add(pointers_cap * size_of::<FatPointer>())
            .saturating_add(pointers_refs_cap * size_of::<ThinPointer>())
            .saturating_add(blobs_cap)
            .saturating_add(inodes_cap * size_of::<Inode>());

        StorageMemoryUsage {
            nodes_len: self.nodes.len(),
            nodes_cap,
            directories_len: self.directories.len(),
            directories_cap,
            temp_dir_cap,
            temp_inodes_index: temp_inodes_index_cap,
            blobs_len: self.blobs.len(),
            blobs_cap,
            inodes_len: self.inodes.len(),
            inodes_cap,
            pointers_len: self.fat_pointers.len(),
            pointers_cap,
            pointers_refs_cap,
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

    pub fn set_hashid_of_pointer(
        &self,
        pointer: &FatPointer,
        hash_id: HashId,
    ) -> Result<(), StorageError> {
        let mut pointers_data = self.pointers_data.borrow_mut();

        let ptr_id = pointer
            .ptr_id()
            .ok_or(StorageError::InodePointerIdNotFound)?
            .as_u64();

        let object_ref = pointers_data.entry(ptr_id).or_default();
        object_ref.hash_id.replace(hash_id);

        Ok(())
    }

    pub fn set_offset_pointer(
        &self,
        pointer: &FatPointer,
        offset: AbsoluteOffset,
    ) -> Result<(), StorageError> {
        let mut pointers_data = self.pointers_data.borrow_mut();

        let ptr_id = pointer
            .ptr_id()
            .ok_or(StorageError::InodePointerIdNotFound)?
            .as_u64();

        let object_ref = pointers_data.entry(ptr_id).or_default();
        object_ref.offset.replace(offset);

        Ok(())
    }

    pub fn set_pointer_data(
        &self,
        pointer: &FatPointer,
        object_ref: ObjectReference,
    ) -> Result<(), StorageError> {
        let mut pointers_data = self.pointers_data.borrow_mut();

        let ptr_id = pointer
            .ptr_id()
            .ok_or(StorageError::InodePointerIdNotFound)?
            .as_u64();

        let old = pointers_data.insert(ptr_id, object_ref);
        debug_assert!(old.is_none());

        Ok(())
    }

    pub fn pointer_retrieve_offset(
        &self,
        pointer: &FatPointer,
    ) -> Result<Option<AbsoluteOffset>, StorageError> {
        if let Some(offset) = pointer.get_data().and_then(|r| r.offset_opt()) {
            return Ok(Some(offset));
        }

        let ptr_id = pointer
            .ptr_id()
            .ok_or(StorageError::InodePointerIdNotFound)?
            .as_u64();

        let refs = self.pointers_data.borrow();
        let object_ref = refs.get(&ptr_id).unwrap();

        Ok(object_ref.offset_opt())
    }

    pub fn retrieve_hashid_of_pointer(
        &self,
        pointer: &FatPointer,
        repository: &ContextKeyValueStore,
    ) -> Result<Option<HashId>, HashingError> {
        let mut refs = self.pointers_data.borrow_mut();

        let offset = match pointer.get_data() {
            Some(object_ref) => {
                if let Some(hash_id) = object_ref.hash_id_opt() {
                    return Ok(Some(hash_id));
                }
                object_ref.offset()
            }
            None => {
                let ptr_id = pointer.ptr_id().unwrap().as_u64();
                let object_ref = refs.entry(ptr_id).or_default();

                if let Some(hash_id) = object_ref.hash_id_opt() {
                    return Ok(Some(hash_id));
                }

                match object_ref.offset {
                    Some(offset) => offset,
                    None => return Ok(None),
                }
            }
        };

        let hash_id = match self.offsets_to_hash_id.get(&offset) {
            Some(hash_id) => *hash_id,
            None => {
                let object_ref = ObjectReference::new(None, Some(offset));
                repository.get_hash_id(object_ref)?
            }
        };

        let ptr_id = match pointer.ptr_id() {
            Some(ptr_id) => ptr_id.as_u64(),
            None => return Ok(Some(hash_id)),
        };

        let object_ref = refs.entry(ptr_id).or_default();
        object_ref.hash_id.replace(hash_id);

        Ok(Some(hash_id))
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
    // #[cfg(test)]
    pub fn get_owned_dir(
        &mut self,
        dir_id: DirectoryId,
        strings: &mut StringInterner,
        repository: &ContextKeyValueStore,
    ) -> Option<Vec<(String, DirEntry)>> {
        let dir = self.dir_to_vec_sorted(dir_id, strings, repository).ok()?;

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
        &mut self,
        ptr_id: DirectoryOrInodeId,
        key: &str,
        strings: &mut StringInterner,
        repository: &ContextKeyValueStore,
    ) -> Result<Option<DirEntryId>, StorageError> {
        match ptr_id {
            DirectoryOrInodeId::Directory(dir_id) => {
                let dir_id = dir_id;
                self.dir_find_dir_entry(dir_id, key, strings, repository)
            }
            DirectoryOrInodeId::Inode(inode_id) => {
                let Inode {
                    depth,
                    pointers,
                    nchildren: _,
                } = self.get_inode(inode_id)?;

                let index_at_depth = index_of_key(*depth as u32, key) as usize;

                let ref_index = match pointers.get_index_for_ptr(index_at_depth) {
                    Some(index) => index,
                    None => return Ok(None),
                };

                let ptr_id = if let Some(ptr_id) = self.pointer_get_id(ref_index) {
                    ptr_id
                } else {
                    self.pointer_fetch(ref_index, repository, strings)?
                };

                // let inode_id = pointer.inode_id();
                self.dir_find_dir_entry_recursive(ptr_id, key, strings, repository)
            }
        }

        // let inode = self.get_inode(inode_id)?;

        // match inode {
        //     Inode::Directory(dir_id) => {
        //         let dir_id = *dir_id;
        //         self.dir_find_dir_entry(dir_id, key, strings, repository)
        //     }
        //     Inode::Pointers {
        //         depth, pointers, ..
        //     } => {
        //         let index_at_depth = index_of_key(*depth, key) as usize;

        //         let pointer = match pointers.get(index_at_depth) {
        //             Some(Some(ref pointer)) => pointer,
        //             Some(None) | None => return Ok(None),
        //         };

        //         let inode_id = if let Some(inode_id) = pointer.inode_id() {
        //             inode_id
        //         } else {
        //             let pointer_inode_id = repository
        //                 .get_inode(pointer.get_reference(), self, strings)
        //                 .map_err(|_| StorageError::InodeInRepositoryNotFound)?;

        //             self.set_pointer_inode_id(inode_id, index_at_depth, pointer_inode_id)?;
        //             pointer_inode_id
        //         };

        //         // let inode_id = pointer.inode_id();
        //         self.dir_find_dir_entry_recursive(inode_id, key, strings, repository)
        //     }
        // }
    }

    /// Find `key` in the directory.
    pub fn dir_find_dir_entry(
        &mut self,
        dir_id: DirectoryId,
        key: &str,
        strings: &mut StringInterner,
        repository: &ContextKeyValueStore,
    ) -> Result<Option<DirEntryId>, StorageError> {
        if let Some(inode_id) = dir_id.get_inode_id() {
            self.dir_find_dir_entry_recursive(
                DirectoryOrInodeId::Inode(inode_id),
                key,
                strings,
                repository,
            )
        } else {
            let dir = self.get_small_dir(dir_id)?;
            match self.binary_search_in_dir(dir.as_ref(), key, strings)?.ok() {
                Some(index) => Ok(Some(dir[index].1)),
                None => Ok(None),
            }
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
            // Must fit in 31 bits (See Pointer)
            return Err(StorageError::InodeIndexTooBig);
        }

        Ok(current)
    }

    pub fn add_inode_pointers(
        &mut self,
        depth: u16,
        nchildren: u32,
        pointers: [Option<PointerOnStack>; 32],
    ) -> Result<DirectoryOrInodeId, StorageError> {
        let start = self.thin_pointers.len() as u32;
        let pointers_id = PointersId::from((start, &pointers));

        for (_, pointer) in pointers.iter().enumerate() {
            let pointer = match pointer {
                Some(pointer) => pointer,
                None => continue,
            };

            let thin_pointer: ThinPointer = if let Some(DirectoryOrInodeId::Inode(inode_id)) =
                pointer.ptr_id()
            {
                let inode_id: u32 = inode_id.into();
                ThinPointer::new()
                    .with_ref_kind(ThinPointerKind::InodeId)
                    .with_is_commited(pointer.is_commited())
                    .with_value(inode_id)
            } else if let Some(thin_pointer) = pointer.thin_pointer.clone() {
                // `ThinPointer` already exist, use it.
                // This avoid growing `Self::pointers`.
                thin_pointer
            } else {
                // Create a new `Pointer`
                let fat_pointer: PointerId = self.fat_pointers.push(pointer.fat_pointer.clone())?;
                let fat_pointer: u32 = fat_pointer.into();
                ThinPointer::new()
                    .with_ref_kind(ThinPointerKind::BigPointer)
                    .with_is_commited(pointer.is_commited())
                    .with_value(fat_pointer)
            };

            self.thin_pointers.push(thin_pointer);
        }

        let current = self.inodes.push(Inode {
            depth,
            nchildren,
            pointers: pointers_id,
        })?;

        Ok(DirectoryOrInodeId::Inode(current))
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

    fn prepare_temp_hash_inodes(&mut self, dir_range: &TempDirRange) {
        if self.temp_inodes_index.capacity() == 0 {
            self.temp_inodes_index = Vec::with_capacity(1024);
        }

        if self.temp_inodes_index.len() < dir_range.end {
            // Make sure that `Self::temp_inodes_index` is at least the same
            // size than `Self::temp_dir[dir_range]`.
            self.temp_inodes_index.resize(dir_range.end, 0);
        }
    }

    fn create_inode(
        &mut self,
        depth: u16,
        dir_range: TempDirRange,
        strings: &StringInterner,
    ) -> Result<DirectoryOrInodeId, StorageError> {
        let dir_range_len = dir_range.end - dir_range.start;

        // println!("CREATE_INODE {:?} {:?}", dir_range, dir_range_len);

        if dir_range_len <= INODE_POINTER_THRESHOLD {
            // The directory in `dir_range` is not guaranted to be sorted.
            // We use `Self::copy_sorted` to copy that directory into `Storage::directories` in
            // a sorted order.

            let new_dir_id = self.copy_sorted(dir_range, strings)?;

            // self.add_inode(Inode::Directory(new_dir_id))

            Ok(DirectoryOrInodeId::Directory(new_dir_id))
        } else {
            let nchildren = dir_range_len as u32;
            let mut pointers: [Option<PointerOnStack>; 32] = Default::default();

            self.prepare_temp_hash_inodes(&dir_range);

            // Compute the hashes of the whole `dir_range`
            // We put them in `Self::temp_inodes_index` to retrieve them later
            for i in dir_range.clone() {
                let (key_id, _) = self.temp_dir[i];
                let key = strings.get_str(key_id)?;

                let index = index_of_key(depth as u32, &key) as u8;
                self.temp_inodes_index[i] = index;
                // The index (hash) of `Self::temp_dir[i]` is now
                // at `Self::temp_inodes_index[i]`
            }

            for index in 0..32u8 {
                // Put all the entries with the same index (hash) in a continous
                // range
                let range = self.with_temp_dir_range(|this| {
                    for i in dir_range.clone() {
                        // Retrieve the index we computed above
                        // The index of `Self::temp_dir[i]` is at `Self::temp_inodes_index[i]`
                        if this.temp_inodes_index[i] != index {
                            continue;
                        }
                        this.temp_dir.push(this.temp_dir[i]);
                    }
                    Ok(())
                })?;

                if range.is_empty() {
                    continue;
                }

                let dir_or_inode_id = self.create_inode(depth + 1, range, strings)?;

                pointers[index as usize] = Some(PointerOnStack {
                    thin_pointer: None,
                    fat_pointer: FatPointer::new(dir_or_inode_id),
                });
                // pointers[index as usize] = Some(PointerWithInfo {
                //     index: 0,
                //     object_ref: Default::default(),
                //     // hash_id: None,
                //     // offset: None,
                //     pointer: Pointer::new(None, dir_or_inode_id),
                // });
            }

            self.add_inode_pointers(depth, nchildren, pointers)
            // self.add_inode(Inode::Pointers {
            //     depth,
            //     nchildren,
            //     npointers,
            //     pointers,
            // })
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

    pub fn into_pointers_on_stack(
        &self,
        pointers: PointersId,
    ) -> Result<[Option<PointerOnStack>; 32], StorageError> {
        let mut cloned: [Option<PointerOnStack>; 32] = Default::default();

        for (ptr_index, index) in pointers.iter() {
            let thin_pointer = self.thin_pointers.get(index).cloned().unwrap();

            cloned[ptr_index] = Some(PointerOnStack {
                thin_pointer: Some(thin_pointer.clone()),
                fat_pointer: FatPointer::from_thin_pointer(thin_pointer, self)?,
            });
        }

        Ok(cloned)
    }

    pub fn pointer_get_id(&self, thin_pointer_index: usize) -> Option<DirectoryOrInodeId> {
        let thin_pointer = self.thin_pointers.get(thin_pointer_index).unwrap();

        match thin_pointer.get_value() {
            ThinPointerValue::Inode(inode_id) => Some(DirectoryOrInodeId::Inode(inode_id)),
            ThinPointerValue::BigPointer(pointer_id) => {
                let pointer = self.fat_pointers.get(pointer_id).unwrap().unwrap();
                pointer.ptr_id()
            }
        }
    }

    pub fn pointer_copy(&self, thin_pointer_index: usize) -> Option<FatPointer> {
        let thin_pointer = self.thin_pointers.get(thin_pointer_index).unwrap();

        match thin_pointer.get_value() {
            ThinPointerValue::Inode(inode_id) => {
                let ptr = FatPointer::new(DirectoryOrInodeId::Inode(inode_id));
                ptr.set_commited(thin_pointer.is_commited());
                Some(ptr)
            }
            ThinPointerValue::BigPointer(pointer_id) => {
                let pointer = self.fat_pointers.get(pointer_id).unwrap().unwrap();
                Some(pointer.clone())
            }
        }
    }

    fn pointer_fetch_on_stack(
        &mut self,
        pointer: &PointerOnStack,
        repository: &ContextKeyValueStore,
        strings: &mut StringInterner,
    ) -> Result<DirectoryOrInodeId, StorageError> {
        let pointer_data = pointer.get_data().unwrap();

        let pointer_inode_id = repository
            .get_inode(pointer_data, self, strings)
            .map_err(|_| StorageError::InodeInRepositoryNotFound)?;

        pointer.set_ptr_id(pointer_inode_id);

        self.set_pointer_data(pointer, pointer_data)?;

        Ok(pointer_inode_id)
    }

    fn pointer_fetch(
        &mut self,
        thin_pointer_index: usize,
        repository: &ContextKeyValueStore,
        strings: &mut StringInterner,
    ) -> Result<DirectoryOrInodeId, StorageError> {
        let thin_pointer = self.thin_pointers.get(thin_pointer_index).unwrap();

        let pointer_id = match thin_pointer.get_value() {
            ThinPointerValue::Inode(_) => panic!(),
            ThinPointerValue::BigPointer(pointer_index) => pointer_index,
        };

        let pointer = self.fat_pointers.get(pointer_id)?.unwrap();
        let pointer_data = pointer.get_data().unwrap();

        let pointer_inode_id = repository
            .get_inode(pointer_data, self, strings)
            .map_err(|_| StorageError::InodeInRepositoryNotFound)?;

        let pointer = self.fat_pointers.get(pointer_id)?.unwrap();

        pointer.set_ptr_id(pointer_inode_id);
        self.set_pointer_data(pointer, pointer_data)?;

        Ok(pointer_inode_id)
    }

    #[allow(clippy::too_many_arguments)]
    fn insert_inode(
        &mut self,
        depth: u16,
        ptr_id: DirectoryOrInodeId,
        key: &str,
        key_id: StringId,
        dir_entry: DirEntry,
        strings: &mut StringInterner,
        repository: &ContextKeyValueStore,
    ) -> Result<(DirectoryOrInodeId, IsNewKey), StorageError> {
        // let inode = self.get_inode(inode_id)?;

        match ptr_id {
            DirectoryOrInodeId::Directory(dir_id) => {
                let dir_id = dir_id;
                let dir_entry_id = self.add_dir_entry(dir_entry)?;

                // println!("INSERT_INODE DIR {:?}", dir_id);

                // Copy the existing directory into `Self::temp_dir` to create an inode
                let range = self.with_temp_dir_range(|this| {
                    let range = this.copy_dir_in_temp_dir(dir_id)?;

                    // We're using `Vec::insert` below and we don't want to invalidate
                    // any existing `TempDirRange`
                    debug_assert_eq!(range.end, this.temp_dir.len());

                    // println!("LA {:?} {:?}", key_id, &this.temp_dir[range.clone()]);

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
            DirectoryOrInodeId::Inode(inode_id) => {
                let Inode {
                    depth,
                    nchildren,
                    pointers,
                } = self.get_inode(inode_id)?;

                let nchildren = *nchildren;
                let depth = *depth;

                let index_at_depth = index_of_key(depth as u32, key) as usize;

                let mut pointers = self.into_pointers_on_stack(*pointers)?;

                let (inode_id, is_new_key) = if let Some(pointer) = &pointers[index_at_depth] {
                    let ptr_id = match pointer.ptr_id() {
                        Some(ptr_id) => ptr_id,
                        None => self.pointer_fetch_on_stack(pointer, repository, strings)?,
                    };

                    let depth = depth + 1;
                    self.insert_inode(depth, ptr_id, key, key_id, dir_entry, strings, repository)?
                } else {
                    let new_dir_id = self.insert_dir_single_dir_entry(key_id, dir_entry)?;
                    let inode_id = self.create_inode(depth, new_dir_id, strings)?;
                    (inode_id, true)
                };

                // pointers[index_at_depth] = Some(PointerWithInfo {
                //     index: 0,
                //     object_ref: Default::default(),
                //     // hash_id: None,
                //     // offset: None,
                //     pointer: Pointer::new(None, inode_id),
                // });

                // println!("NEW_KEY={:?} {:?}", is_new_key, inode_id);

                pointers[index_at_depth] = Some(PointerOnStack {
                    thin_pointer: None,
                    fat_pointer: FatPointer::new(inode_id),
                });

                let nchildren = if is_new_key { nchildren + 1 } else { nchildren };

                let ptr_id = self.add_inode_pointers(depth, nchildren, pointers)?;

                // let inode_id = self.add_inode(Inode::Pointers {
                //     depth,
                //     nchildren: if is_new_key { nchildren + 1 } else { nchildren },
                //     npointers,
                //     pointers,
                // })?;

                Ok((ptr_id, is_new_key))
            }
        }
    }

    // fn insert_inode(
    //     &mut self,
    //     depth: u32,
    //     inode_id: InodeId,
    //     key: &str,
    //     key_id: StringId,
    //     dir_entry: DirEntry,
    //     strings: &mut StringInterner,
    //     repository: &ContextKeyValueStore,
    // ) -> Result<(InodeId, IsNewKey), StorageError> {
    //     let inode = self.get_inode(inode_id)?;

    //     match inode {
    //         Inode::Directory(dir_id) => {
    //             let dir_id = *dir_id;
    //             let dir_entry_id = self.add_dir_entry(dir_entry)?;

    //             // Copy the existing directory into `Self::temp_dir` to create an inode
    //             let range = self.with_temp_dir_range(|this| {
    //                 let range = this.copy_dir_in_temp_dir(dir_id)?;

    //                 // We're using `Vec::insert` below and we don't want to invalidate
    //                 // any existing `TempDirRange`
    //                 debug_assert_eq!(range.end, this.temp_dir.len());

    //                 let start = range.start;
    //                 match this.binary_search_in_dir(&this.temp_dir[range], key, strings)? {
    //                     Ok(found) => this.temp_dir[start + found] = (key_id, dir_entry_id),
    //                     Err(index) => this.temp_dir.insert(start + index, (key_id, dir_entry_id)),
    //                 }

    //                 Ok(())
    //             })?;

    //             let new_inode_id = self.create_inode(depth, range, strings)?;
    //             let is_new_key = self.inode_len(new_inode_id)? != dir_id.small_dir_len();

    //             Ok((new_inode_id, is_new_key))
    //         }
    //         Inode::Pointers {
    //             depth,
    //             nchildren,
    //             mut npointers,
    //             pointers,
    //         } => {
    //             let mut pointers = pointers.clone();
    //             let nchildren = *nchildren;
    //             let depth = *depth;

    //             let index_at_depth = index_of_key(depth, key) as usize;

    //             let (inode_id, is_new_key) = if let Some(pointer) = &pointers[index_at_depth] {
    //                 let inode_id = match pointer.inode_id() {
    //                     Some(inode_id) => inode_id,
    //                     None => {
    //                         let inode_id = repository
    //                             .get_inode(pointer.get_reference(), self, strings)
    //                             .map_err(|_| StorageError::InodeInRepositoryNotFound)?;

    //                         pointer.set_inode_id(inode_id);
    //                         inode_id
    //                     }
    //                 };

    //                 self.insert_inode(
    //                     depth + 1,
    //                     inode_id,
    //                     key,
    //                     key_id,
    //                     dir_entry,
    //                     strings,
    //                     repository,
    //                 )?
    //             } else {
    //                 npointers += 1;

    //                 let new_dir_id = self.insert_dir_single_dir_entry(key_id, dir_entry)?;
    //                 let inode_id = self.create_inode(depth, new_dir_id, strings)?;
    //                 (inode_id, true)
    //             };

    //             pointers[index_at_depth] = Some(Pointer::new(None, inode_id));

    //             let inode_id = self.add_inode(Inode::Pointers {
    //                 depth,
    //                 nchildren: if is_new_key { nchildren + 1 } else { nchildren },
    //                 npointers,
    //                 pointers,
    //             })?;

    //             Ok((inode_id, is_new_key))
    //         }
    //     }
    // }

    /// [test only] Remove hash ids in the inode and it's children
    ///
    /// This is used to force recomputing hashes
    #[cfg(test)]
    pub fn inodes_drop_hash_ids(&mut self, inode_id: InodeId) {
        let inode = self.get_inode(inode_id).unwrap();

        // TODO: Find a way to iterates, without using Cow::Owned
        // let pointers = self.get_pointers_ref(inode.pointers);

        for (_, index) in inode.pointers.iter() {
            let ptr_id = self.pointer_get_id(index).unwrap();

            // let pointer = self.pointers.get(index).unwrap();

            // let ptr_id = pointer.ptr_id().unwrap().as_u64();
            self.pointers_data.borrow_mut().remove(&ptr_id.as_u64());
            // pointer.set_hash_id(None);

            if let DirectoryOrInodeId::Inode(inode_id) = ptr_id {
                self.inodes_drop_hash_ids(inode_id);
            }
        }

        // for pointer in pointers.iter() {
        //     pointer.set_hash_id(None);

        //     if let Some(DirectoryOrInodeId::Inode(inode_id)) = pointer.ptr_id() {
        //         self.inodes_drop_hash_ids(inode_id);
        //     }
        // }

        // if let Inode::Pointers { pointers, .. } = inode {
        //     for pointer in pointers.iter().filter_map(|p| p.as_ref()) {
        //         pointer.set_hash_id(None);

        //         if let Some(inode_id) = pointer.inode_id() {
        //             self.inodes_drop_hash_ids(inode_id);
        //         }
        //     }
        // };
    }

    fn iter_full_inodes_recursive_unsorted<Fun>(
        &mut self,
        ptr_id: DirectoryOrInodeId,
        strings: &mut StringInterner,
        repository: &ContextKeyValueStore,
        fun: &mut Fun,
    ) -> Result<(), MerkleError>
    where
        Fun: FnMut(&(StringId, DirEntryId)) -> Result<(), MerkleError>,
    {
        match ptr_id {
            DirectoryOrInodeId::Directory(dir_id) => {
                let dir = self.get_small_dir(dir_id)?;
                for elem in dir.as_ref() {
                    fun(elem)?;
                }
            }
            DirectoryOrInodeId::Inode(inode_id) => {
                let inode = self.get_inode(inode_id)?;

                for (_, ref_index) in inode.pointers.iter() {
                    let ptr_id = if let Some(ptr_id) = self.pointer_get_id(ref_index) {
                        ptr_id
                    } else {
                        self.pointer_fetch(ref_index, repository, strings)?
                    };

                    self.iter_full_inodes_recursive_unsorted(ptr_id, strings, repository, fun)?;
                }
            }
        }

        Ok(())
    }

    // fn iter_full_inodes_recursive_unsorted<Fun>(
    //     &mut self,
    //     inode_id: InodeId,
    //     strings: &mut StringInterner,
    //     repository: &ContextKeyValueStore,
    //     fun: &mut Fun,
    // ) -> Result<(), MerkleError>
    // where
    //     Fun: FnMut(&(StringId, DirEntryId)) -> Result<(), MerkleError>,
    // {
    //     let inode = self.get_inode(inode_id)?.clone();

    //     match inode {
    //         Inode::Pointers { pointers, .. } => {
    //             // for pointer in pointers.iter().filter_map(|p| p.as_ref()) {
    //             for (index, pointer) in pointers.iter().enumerate() {
    //                 let pointer = match pointer {
    //                     Some(pointer) => pointer,
    //                     None => continue,
    //                 };

    //                 let inode_id = match pointer.inode_id() {
    //                     Some(inode_id) => inode_id,
    //                     None => {
    //                         let pointer_inode_id = repository
    //                             .get_inode(pointer.get_reference(), self, strings)
    //                             .map_err(|_| StorageError::InodeInRepositoryNotFound)?;

    //                         self.set_pointer_inode_id(inode_id, index, pointer_inode_id)?;
    //                         pointer_inode_id
    //                     }
    //                 };

    //                 self.iter_full_inodes_recursive_unsorted(inode_id, strings, repository, fun)?;
    //             }
    //         }
    //         Inode::Directory(dir_id) => {
    //             let dir = self.get_small_dir(dir_id)?;
    //             for elem in dir.as_ref() {
    //                 fun(elem)?;
    //             }
    //         }
    //     };

    //     Ok(())
    // }

    fn iter_inodes_recursive_unsorted<Fun>(
        &self,
        ptr_id: DirectoryOrInodeId,
        fun: &mut Fun,
    ) -> Result<(), MerkleError>
    where
        Fun: FnMut(&(StringId, DirEntryId)) -> Result<(), MerkleError>,
    {
        match ptr_id {
            DirectoryOrInodeId::Directory(dir_id) => {
                let dir = self.get_small_dir(dir_id)?;
                for elem in dir.as_ref() {
                    fun(elem)?;
                }
            }
            DirectoryOrInodeId::Inode(inode_id) => {
                let inode = self.get_inode(inode_id)?;

                // for pointer in self.iter_pointers(inode.pointers) {
                for (_, ref_index) in inode.pointers.iter() {
                    // let pointer = self.pointers_refs.get(index).unwrap();

                    // When the inode is not deserialized, ignore it
                    // See `Self::iter_full_inodes_recursive_unsorted` to iterate on
                    // the full inode
                    if let Some(ptr_id) = self.pointer_get_id(ref_index) {
                        // let inode = self.get_inode(inode_id)?;
                        self.iter_inodes_recursive_unsorted(ptr_id, fun)?;
                    }

                    // let pointer = self.pointers_refs.get(index).unwrap();

                    // // When the inode is not deserialized, ignore it
                    // // See `Self::iter_full_inodes_recursive_unsorted` to iterate on
                    // // the full inode
                    // if let Some(ptr_id) = pointer.ptr_id() {
                    //     // let inode = self.get_inode(inode_id)?;
                    //     self.iter_inodes_recursive_unsorted(ptr_id, fun)?;
                    // }
                }

                // let pointers = self.clone_pointers(inode.pointers);

                // for pointer in pointers.iter().filter_map(|p| p.as_ref()) {
                //     // When the inode is not deserialized, ignore it
                //     // See `Self::iter_full_inodes_recursive_unsorted` to iterate on
                //     // the full inode
                //     if let Some(inode_id) = pointer.inode_id() {
                //         let inode = self.get_inode(inode_id)?;
                //         self.iter_inodes_recursive_unsorted(inode, fun)?;
                //     }
                // }
            }
        }

        Ok(())
    }

    // fn iter_inodes_recursive_unsorted<Fun>(
    //     &self,
    //     inode: &Inode,
    //     fun: &mut Fun,
    // ) -> Result<(), MerkleError>
    // where
    //     Fun: FnMut(&(StringId, DirEntryId)) -> Result<(), MerkleError>,
    // {
    //     match inode {
    //         Inode::Pointers { pointers, .. } => {
    //             for pointer in pointers.iter().filter_map(|p| p.as_ref()) {
    //                 // When the inode is not deserialized, ignore it
    //                 // See `Self::iter_full_inodes_recursive_unsorted` to iterate on
    //                 // the full inode
    //                 if let Some(inode_id) = pointer.inode_id() {
    //                     let inode = self.get_inode(inode_id)?;
    //                     self.iter_inodes_recursive_unsorted(inode, fun)?;
    //                 }
    //             }
    //         }
    //         Inode::Directory(dir_id) => {
    //             let dir = self.get_small_dir(*dir_id)?;
    //             for elem in dir.as_ref() {
    //                 fun(elem)?;
    //             }
    //         }
    //     };

    //     Ok(())
    // }

    fn dir_full_iterate_unsorted<Fun>(
        &mut self,
        dir_id: DirectoryId,
        strings: &mut StringInterner,
        repository: &ContextKeyValueStore,
        mut fun: Fun,
    ) -> Result<(), MerkleError>
    where
        Fun: FnMut(&(StringId, DirEntryId)) -> Result<(), MerkleError>,
    {
        if let Some(inode_id) = dir_id.get_inode_id() {
            self.iter_full_inodes_recursive_unsorted(
                DirectoryOrInodeId::Inode(inode_id),
                strings,
                repository,
                &mut fun,
            )?;
        } else {
            let dir = self.get_small_dir(dir_id)?;
            for elem in dir.as_ref() {
                fun(elem)?;
            }
        }
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
            // let inode = self.get_inode(inode_id)?;

            self.iter_inodes_recursive_unsorted(DirectoryOrInodeId::Inode(inode_id), &mut fun)?;
        } else {
            let dir = self.get_small_dir(dir_id)?;
            for elem in dir.as_ref() {
                fun(elem)?;
            }
        }
        Ok(())
    }

    // fn inode_len(&self, inode_id: InodeId) -> Result<usize, StorageError> {
    //     let inode = self.get_inode(inode_id)?;
    //     match inode {
    //         Inode::Pointers {
    //             nchildren: children,
    //             ..
    //         } => Ok(*children as usize),
    //         Inode::Directory(dir_id) => Ok(dir_id.small_dir_len()),
    //     }
    // }

    fn inode_len(&self, ptr_id: DirectoryOrInodeId) -> Result<usize, StorageError> {
        match ptr_id {
            DirectoryOrInodeId::Directory(dir_id) => Ok(dir_id.small_dir_len()),
            DirectoryOrInodeId::Inode(inode_id) => {
                let inode = self.get_inode(inode_id)?;
                Ok(inode.nchildren as usize)
            }
        }
    }

    /// Return the number of nodes in `dir_id`.
    pub fn dir_len(&self, dir_id: DirectoryId) -> Result<usize, StorageError> {
        if let Some(inode_id) = dir_id.get_inode_id() {
            self.inode_len(DirectoryOrInodeId::Inode(inode_id))
        } else {
            Ok(dir_id.small_dir_len())
        }
    }

    /// Make a vector of `(StringId, DirEntryId)`
    ///
    /// The vector won't be sorted when the underlying directory is an Inode.
    /// `Self::dir_to_vec_sorted` can be used to get the vector sorted.
    pub fn dir_to_vec_unsorted(
        &mut self,
        dir_id: DirectoryId,
        strings: &mut StringInterner,
        repository: &ContextKeyValueStore,
    ) -> Result<Vec<(StringId, DirEntryId)>, MerkleError> {
        let mut vec = Vec::with_capacity(self.dir_len(dir_id)?);

        self.dir_full_iterate_unsorted(dir_id, strings, repository, |&(key_id, dir_entry_id)| {
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
        &mut self,
        dir_id: DirectoryId,
        strings: &mut StringInterner,
        repository: &ContextKeyValueStore,
    ) -> Result<Vec<(StringId, DirEntryId)>, MerkleError> {
        if dir_id.get_inode_id().is_some() {
            let mut dir = self.dir_to_vec_unsorted(dir_id, strings, repository)?;
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
        repository: &ContextKeyValueStore,
    ) -> Result<DirectoryId, StorageError> {
        let key_id = strings.make_string_id(key_str);

        // Are we inserting in an Inode ?
        if let Some(inode_id) = dir_id.get_inode_id() {
            let (inode_id, _) = self.insert_inode(
                0,
                DirectoryOrInodeId::Inode(inode_id),
                key_str,
                key_id,
                dir_entry,
                strings,
                repository,
            )?;

            self.temp_dir.clear();
            self.temp_inodes_index.clear();

            return Ok(inode_id.into_dir()); // TODO
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
            self.temp_inodes_index.clear();

            // TODO
            Ok(inode_id.into_dir())
        }
    }

    fn remove_in_inode_recursive(
        &mut self,
        ptr_id: DirectoryOrInodeId,
        key: &str,
        strings: &mut StringInterner,
        repository: &ContextKeyValueStore,
    ) -> Result<Option<DirectoryOrInodeId>, StorageError> {
        match ptr_id {
            DirectoryOrInodeId::Directory(dir_id) => {
                let new_dir_id = self.dir_remove(dir_id, key, strings, repository)?;

                if new_dir_id.is_empty() {
                    // The directory is now empty, return None to indicate that it
                    // should be removed from the Inode::Pointers
                    Ok(None)
                } else if new_dir_id == dir_id {
                    // The key was not found in the directory, so it's the same directory.
                    // Do not create a new inode.
                    Ok(Some(ptr_id))
                } else {
                    Ok(Some(DirectoryOrInodeId::Directory(new_dir_id)))
                    // self.add_inode(Inode::Directory(new_dir_id)).map(Some)
                }
            }
            DirectoryOrInodeId::Inode(inode_id) => {
                let Inode {
                    depth,
                    nchildren,
                    pointers,
                } = self.get_inode(inode_id)?;

                // let depth = *depth;
                // let mut npointers = *npointers;
                // let nchildren = *nchildren;
                let new_nchildren = nchildren - 1;

                let new_inode_id = if new_nchildren as usize <= INODE_POINTER_THRESHOLD {
                    // After removing an element from this `Inode::Pointers`, it remains
                    // INODE_POINTER_THRESHOLD items, so it should be converted to a
                    // `Inode::Directory`.

                    let dir_id = self.inodes_to_dir_sorted(
                        DirectoryOrInodeId::Inode(inode_id),
                        strings,
                        repository,
                    )?;
                    let new_dir_id = self.dir_remove(dir_id, key, strings, repository)?;

                    if dir_id == new_dir_id {
                        // The key was not found.

                        // Remove the directory that was just created with
                        // Self::inodes_to_dir_sorted above, it won't be used and save space.
                        self.directories.remove_last_nelems(dir_id.small_dir_len());
                        return Ok(Some(ptr_id));
                    }

                    DirectoryOrInodeId::Directory(new_dir_id)
                    // self.add_inode(Inode::Directory(new_dir_id))?
                } else {
                    let mut pointers = self.into_pointers_on_stack(*pointers)?;
                    let index_at_depth = index_of_key(*depth as u32, key) as usize;
                    let depth = *depth;
                    // let mut pointers = pointers.clone();

                    let pointer = match pointers[index_at_depth].as_ref() {
                        Some(pointer) => pointer,
                        None => return Ok(Some(ptr_id)), // The key was not found
                    };

                    let ptr_inode_id = match pointer.ptr_id() {
                        Some(inode_id) => inode_id,
                        None => self.pointer_fetch_on_stack(pointer, repository, strings)?,
                    };

                    match self.remove_in_inode_recursive(ptr_inode_id, key, strings, repository)? {
                        Some(new_ptr_inode_id) if new_ptr_inode_id == ptr_inode_id => {
                            // The key was not found, don't create a new inode
                            return Ok(Some(ptr_id));
                        }
                        Some(new_ptr_inode_id) => {
                            pointers[index_at_depth] = Some(PointerOnStack {
                                thin_pointer: None,
                                fat_pointer: FatPointer::new(new_ptr_inode_id),
                            });
                            // pointers[index_at_depth] = Some(PointerWithInfo {
                            //     index: 0,
                            //     object_ref: Default::default(),
                            //     // hash_id: None,
                            //     // offset: None,
                            //     pointer: Pointer::new(None, new_ptr_inode_id),
                            // });
                        }
                        None => {
                            // The key was removed and it result in an empty directory.
                            // Remove the pointer: make it `None`.
                            pointers[index_at_depth] = None;
                            // npointers -= 1; TODO: Is it correct ?
                        }
                    }

                    self.add_inode_pointers(depth, new_nchildren, pointers)?

                    // self.add_inode(Inode::Pointers {
                    //     depth,
                    //     nchildren: new_nchildren,
                    //     npointers,
                    //     pointers,
                    // })?
                };

                Ok(Some(new_inode_id))
            }
        }

        // let inode = self.get_inode(inode_id)?;

        // match inode {
        //     Inode::Directory(dir_id) => {
        //         let dir_id = *dir_id;
        //         let new_dir_id = self.dir_remove(dir_id, key, strings, repository)?;

        //         if new_dir_id.is_empty() {
        //             // The directory is now empty, return None to indicate that it
        //             // should be removed from the Inode::Pointers
        //             Ok(None)
        //         } else if new_dir_id == dir_id {
        //             // The key was not found in the directory, so it's the same directory.
        //             // Do not create a new inode.
        //             Ok(Some(inode_id))
        //         } else {
        //             self.add_inode(Inode::Directory(new_dir_id)).map(Some)
        //         }
        //     }
        //     Inode::Pointers {
        //         depth,
        //         nchildren,
        //         npointers,
        //         pointers,
        //     } => {
        //         let depth = *depth;
        //         let mut npointers = *npointers;
        //         let nchildren = *nchildren;
        //         let new_nchildren = nchildren - 1;

        //         let new_inode_id = if new_nchildren as usize <= INODE_POINTER_THRESHOLD {
        //             // After removing an element from this `Inode::Pointers`, it remains
        //             // INODE_POINTER_THRESHOLD items, so it should be converted to a
        //             // `Inode::Directory`.

        //             let dir_id = self.inodes_to_dir_sorted(inode_id, strings, repository)?;
        //             let new_dir_id = self.dir_remove(dir_id, key, strings, repository)?;

        //             if dir_id == new_dir_id {
        //                 // The key was not found.

        //                 // Remove the directory that was just created with
        //                 // Self::inodes_to_dir_sorted above, it won't be used and save space.
        //                 self.directories.remove_last_nelems(dir_id.small_dir_len());
        //                 return Ok(Some(inode_id));
        //             }

        //             self.add_inode(Inode::Directory(new_dir_id))?
        //         } else {
        //             let index_at_depth = index_of_key(depth, key) as usize;
        //             let mut pointers = pointers.clone();

        //             let pointer = match pointers[index_at_depth].as_ref() {
        //                 Some(pointer) => pointer,
        //                 None => return Ok(Some(inode_id)), // The key was not found
        //             };

        //             let ptr_inode_id = match pointer.inode_id() {
        //                 Some(inode_id) => inode_id,
        //                 None => {
        //                     let pointer_inode_id = repository
        //                         .get_inode(pointer.get_reference(), self, strings)
        //                         .map_err(|_| StorageError::InodeInRepositoryNotFound)?;
        //                     pointer.set_inode_id(pointer_inode_id);
        //                     pointer_inode_id
        //                 }
        //             };

        //             match self.remove_in_inode_recursive(ptr_inode_id, key, strings, repository)? {
        //                 Some(new_ptr_inode_id) if new_ptr_inode_id == ptr_inode_id => {
        //                     // The key was not found, don't create a new inode
        //                     return Ok(Some(inode_id));
        //                 }
        //                 Some(new_ptr_inode_id) => {
        //                     pointers[index_at_depth] = Some(Pointer::new(None, new_ptr_inode_id));
        //                 }
        //                 None => {
        //                     // The key was removed and it result in an empty directory.
        //                     // Remove the pointer: make it `None`.
        //                     pointers[index_at_depth] = None;
        //                     npointers -= 1;
        //                 }
        //             }

        //             self.add_inode(Inode::Pointers {
        //                 depth,
        //                 nchildren: new_nchildren,
        //                 npointers,
        //                 pointers,
        //             })?
        //         };

        //         Ok(Some(new_inode_id))
        //     }
        // }
    }

    /// Convert the Inode into a small directory
    ///
    /// This traverses all elements (all children) of this `inode_id` and
    /// copy them into `Self::directories` in a sorted order.
    fn inodes_to_dir_sorted(
        &mut self,
        ptr_id: DirectoryOrInodeId,
        strings: &mut StringInterner,
        repository: &ContextKeyValueStore,
    ) -> Result<DirectoryId, StorageError> {
        let start = self.temp_dir.len();

        // Iterator on the inodes children and copy all nodes into `Self::temp_dir`
        self.with_new_dir::<_, Result<_, StorageError>>(|this, temp_dir| {
            // let inode = this.get_inode(inode_id)?;

            this.iter_full_inodes_recursive_unsorted(ptr_id, strings, repository, &mut |value| {
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
        strings: &mut StringInterner,
        repository: &ContextKeyValueStore,
    ) -> Result<DirectoryId, StorageError> {
        let inode_id = self.remove_in_inode_recursive(
            DirectoryOrInodeId::Inode(inode_id),
            key,
            strings,
            repository,
        )?;
        let inode_id = inode_id.ok_or(StorageError::RootOfInodeNotAPointer)?;

        if self.inode_len(inode_id)? > DIRECTORY_INODE_THRESHOLD {
            Ok(inode_id.into_dir())
        } else {
            // There is now DIRECTORY_INODE_THRESHOLD or less items:
            // Convert the inode into a 'small' directory
            self.inodes_to_dir_sorted(inode_id, strings, repository)
        }
    }

    /// Remove `key` from the directory and return the new `DirectoryId`.
    ///
    /// If the key doesn't exist, it returns the same `DirectoryId`.
    pub fn dir_remove(
        &mut self,
        dir_id: DirectoryId,
        key: &str,
        strings: &mut StringInterner,
        repository: &ContextKeyValueStore,
    ) -> Result<DirectoryId, StorageError> {
        if let Some(inode_id) = dir_id.get_inode_id() {
            return self.remove_in_inode(inode_id, key, strings, repository);
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
        self.temp_inodes_index = Vec::new();
        self.blobs = ChunkedVec::empty();
        self.inodes = IndexMap::empty();
        self.data = Vec::new();
        self.offsets_to_hash_id = HashMap::default();
        self.fat_pointers = IndexMap::empty();
        self.pointers_data = Default::default();
        // self.pointers_offsets = Default::default();
    }
}

/// Implementation used for `context-tool`
mod tool {
    use super::*;

    impl FatPointer {
        /// Remove all `HashId` and `AbsoluteOffset` in `Self`
        /// This is used in order to recompute them
        ///
        /// Method used for `context-tool` only.
        pub fn forget_reference(&self) {
            let mut inner = self.inner.get();
            // inner.set_hash_id(0);
            inner.set_is_commited(false);
            // inner.set_offset(0);
            self.inner.set(inner);
        }
    }

    impl ThinPointer {
        /// Remove all `HashId` and `AbsoluteOffset` in `Self`
        /// This is used in order to recompute them
        ///
        /// Method used for `context-tool` only.
        pub fn forget_reference(&mut self) {
            self.set_is_commited(false);
        }
    }

    impl Storage {
        /// Return a new `StringInterner` containing only the strings used by `Self`
        /// All the strings in `Self` will now refers to strings inside the new `StringInterner`
        ///
        /// Method used for `context-tool` only.
        pub fn strip_string_interner(&mut self, old_strings: StringInterner) -> StringInterner {
            let mut new_string_interner = StringInterner::default();

            let new_directories: Vec<_> = self
                .directories
                .iter()
                .map(|(string_id, dir_entry_id)| {
                    let s = old_strings.get_str(*string_id).unwrap();
                    let new_id = new_string_interner.make_string_id(s.as_ref());
                    (new_id, *dir_entry_id)
                })
                .collect();

            self.directories = ChunkedVec::with_chunk_capacity(DEFAULT_DIRECTORIES_CAPACITY);
            self.directories.extend_from_slice(&new_directories);

            new_string_interner
        }

        /// Scan the directories (including inodes) and remove duplicates
        /// Duplicates will now have the same `HashId`
        ///
        /// Method used for `context-tool` only.
        pub fn deduplicate_hashes(
            &mut self,
            repository: &ContextKeyValueStore,
        ) -> Result<(), StorageError> {
            let mut unique: HashMap<ObjectHash, HashId> = HashMap::default();

            for (_, dir_entry_id) in self.directories.iter() {
                let dir_entry = self.get_dir_entry(*dir_entry_id).unwrap();

                let hash_id: HashId = match dir_entry.hash_id() {
                    Some(hash_id) => hash_id,
                    None => continue,
                };

                let hash: ObjectHash = repository.get_hash(hash_id.into()).unwrap().into_owned();
                let new_hash_id: HashId = *unique.entry(hash).or_insert(hash_id);

                dir_entry.set_hash_id(new_hash_id);
            }

            for index in 0..self.thin_pointers.len() {
                let pointer = self.pointer_copy(index).unwrap();

                // let hash_id = pointer.hash_id(storage, repository)?.ok_or(MissingHashId)?;

                let hash_id = match self
                    .retrieve_hashid_of_pointer(&pointer, repository)
                    .unwrap()
                {
                    Some(hash_id) => hash_id,
                    None => continue,
                };

                let hash: ObjectHash = repository.get_hash(hash_id.into()).unwrap().into_owned();
                let new_hash_id: HashId = *unique.entry(hash).or_insert(hash_id);

                self.set_hashid_of_pointer(&pointer, new_hash_id)?;
                // pointer.set_hash_id(Some(new_hash_id));
            }

            Ok(())
        }

        /// Remove all `HashId` and `AbsoluteOffset` in `Self`
        /// This is used in order to recompute them
        ///
        /// Method used for `context-tool` only.
        pub fn forget_references(&mut self) {
            self.offsets_to_hash_id = Default::default();

            for (_, dir_entry_id) in self.directories.iter() {
                let dir_entry = self.get_dir_entry(*dir_entry_id).unwrap();
                dir_entry.set_offset(None);
                dir_entry.set_hash_id(None);
                dir_entry.set_commited(false);
            }

            self.pointers_data = Default::default();

            for p in self.fat_pointers.iter_values() {
                p.forget_reference();
            }

            for i in 0..self.thin_pointers.len() {
                self.thin_pointers[i].forget_reference();
            }

            // for inode in self.inodes.iter_values() {
            //     if let Inode::Pointers { pointers, .. } = inode {
            //         for ptr in pointers.iter().filter_map(|p| p.as_ref()) {
            //             ptr.forget_reference();
            //         }
            //     };
            // }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        kv_store::in_memory::InMemory,
        working_tree::{DirEntryKind::Blob, Object},
    };

    use super::*;

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

    #[test]
    fn test_pointers_bitfield() {
        let mut bitfield = PointersBitfield::default();

        bitfield.set(0);
        bitfield.set(1);
        bitfield.set(3);
        bitfield.set(4);
        bitfield.set(8);
        bitfield.set(30);
        bitfield.set(31);

        assert_eq!(bitfield.get_index_for(0).unwrap(), 0);
        assert_eq!(bitfield.get_index_for(1).unwrap(), 1);
        assert_eq!(bitfield.get_index_for(3).unwrap(), 2);
        assert_eq!(bitfield.get_index_for(4).unwrap(), 3);
        assert_eq!(bitfield.get_index_for(8).unwrap(), 4);
        assert_eq!(bitfield.get_index_for(30).unwrap(), 5);
        assert_eq!(bitfield.get_index_for(31).unwrap(), 6);

        assert!(bitfield.get_index_for(15).is_none());
        assert!(bitfield.get_index_for(29).is_none());

        assert_eq!(bitfield.count(), 7);

        let mut bitfield = PointersBitfield::default();

        bitfield.set(5);
        bitfield.set(30);
        bitfield.set(31);

        assert_eq!(bitfield.get_index_for(5).unwrap(), 0);
        assert_eq!(bitfield.get_index_for(30).unwrap(), 1);
        assert_eq!(bitfield.get_index_for(31).unwrap(), 2);

        assert_eq!(bitfield.count(), 3);
    }

    #[test]
    fn test_pointers_id_to_u64() {
        let dir_id = DirectoryId::from(101);
        let id = DirectoryOrInodeId::Directory(dir_id);
        let id_u64 = id.as_u64();
        assert_eq!(dir_id, DirectoryId::from(id_u64));

        let inode_id = InodeId::try_from(101usize).unwrap();
        let id = DirectoryOrInodeId::Inode(inode_id);
        let id_u64 = id.as_u64();
        assert_eq!(inode_id, DirectoryId::from(id_u64).get_inode_id().unwrap());
    }

    #[test]
    fn test_storage() {
        let mut storage = Storage::new();
        let mut strings = StringInterner::default();
        let repo = InMemory::try_new().unwrap();

        let blob_id = storage.add_blob_by_ref(&[1]).unwrap();
        let object = Object::Blob(blob_id);

        let blob2_id = storage.add_blob_by_ref(&[2]).unwrap();
        let object2 = Object::Blob(blob2_id);

        let dir_entry1 = DirEntry::new(Blob, object);
        let dir_entry2 = DirEntry::new(Blob, object2);

        let dir_id = DirectoryId::empty();
        let dir_id = storage
            .dir_insert(dir_id, "a", dir_entry1.clone(), &mut strings, &repo)
            .unwrap();
        let dir_id = storage
            .dir_insert(dir_id, "b", dir_entry2.clone(), &mut strings, &repo)
            .unwrap();
        let dir_id = storage
            .dir_insert(dir_id, "0", dir_entry1.clone(), &mut strings, &repo)
            .unwrap();

        assert_eq!(
            storage.get_owned_dir(dir_id, &mut strings, &repo).unwrap(),
            &[
                ("0".to_string(), dir_entry1.clone()),
                ("a".to_string(), dir_entry1),
                ("b".to_string(), dir_entry2),
            ]
        );
    }

    #[test]
    fn test_pointers_id() {
        let mut bitfield = PointersBitfield::default();

        bitfield.set(0);
        bitfield.set(1);
        bitfield.set(3);
        bitfield.set(4);
        bitfield.set(8);
        bitfield.set(30);
        bitfield.set(31);

        for start in 0..100usize {
            let id = PointersId {
                start: start as u32,
                bitfield,
            };

            let mut iter = InodePointersIter::new(id);

            assert_eq!(iter.next().unwrap(), (0, start));
            assert_eq!(iter.next().unwrap(), (1, start + 1));
            assert_eq!(iter.next().unwrap(), (3, start + 2));
            assert_eq!(iter.next().unwrap(), (4, start + 3));
            assert_eq!(iter.next().unwrap(), (8, start + 4));
            assert_eq!(iter.next().unwrap(), (30, start + 5));
            assert_eq!(iter.next().unwrap(), (31, start + 6));
            assert!(iter.next().is_none());
        }

        let mut bitfield = PointersBitfield::default();
        bitfield.set(5);
        bitfield.set(30);
        bitfield.set(31);

        let id = PointersId { start: 0, bitfield };

        let mut iter = InodePointersIter::new(id);

        assert_eq!(iter.next().unwrap(), (5, 0));
        assert_eq!(iter.next().unwrap(), (30, 1));
        assert_eq!(iter.next().unwrap(), (31, 2));
        assert!(iter.next().is_none());
    }
}
