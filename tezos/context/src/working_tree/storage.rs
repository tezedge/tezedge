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
    collections::{BTreeMap, HashMap},
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
    ContextKeyValueStore, ObjectHash,
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
const FULL_30_BITS: usize = 0x3FFFFFFF;
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
    pub fn try_new_dir(start: usize, end: usize) -> Result<Self, StorageError> {
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
    #[error("InodeIdError: Conversion from usize of a InodeId failed")]
    InodeIdError,
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
    #[error("FatPointerIdError: Conversion from/to usize of a PointerId failed")]
    FatPointerIdError,
    #[error("InvalidHashIdInPointer: The fat pointer contains a null `HashId`")]
    InvalidHashIdInPointer,
    #[error("MissingDataInPointer: The fat pointer is missing its data")]
    MissingDataInPointer,
    #[error("PointerDoesNotHaveData: There is no data for this `FatPointer`")]
    PointerDoesNotHaveData,
    #[error("ThinPointerNotFound: The `ThinPointer` does not exist")]
    ThinPointerNotFound,
    #[error("FatPointerNotFound: The `FatPointer` does not exist")]
    FatPointerNotFound,
}

impl From<DirEntryIdError> for StorageError {
    fn from(_: DirEntryIdError) -> Self {
        Self::DirEntryIdError
    }
}

impl From<InodeIdError> for StorageError {
    fn from(_: InodeIdError) -> Self {
        Self::InodeIdError
    }
}

impl From<FatPointerIdError> for StorageError {
    fn from(_: FatPointerIdError) -> Self {
        Self::FatPointerIdError
    }
}

impl From<std::convert::Infallible> for StorageError {
    fn from(_: std::convert::Infallible) -> Self {
        // This implementation exists only to be able to use `?` on a Result<_, Infallible>
        unreachable!()
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

impl From<DirEntryId> for usize {
    fn from(value: DirEntryId) -> Self {
        value.0 as usize
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

#[derive(Debug, Error)]
#[error("Fail to convert the inode id id from usize")]
pub struct InodeIdError;

impl From<InodeId> for usize {
    fn from(value: InodeId) -> Self {
        value.0 as usize
    }
}

impl TryFrom<usize> for InodeId {
    type Error = InodeIdError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        value.try_into().map(InodeId).map_err(|_| InodeIdError)
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

#[derive(Clone, Debug, Copy, PartialEq, Eq)]
pub struct FatPointerId(u32);

impl From<FatPointerId> for usize {
    fn from(value: FatPointerId) -> Self {
        value.0 as usize
    }
}

impl From<FatPointerId> for u32 {
    fn from(val: FatPointerId) -> Self {
        val.0
    }
}

impl From<u32> for FatPointerId {
    fn from(v: u32) -> Self {
        Self(v)
    }
}

#[derive(Debug, Error)]
#[error("Fail to convert the fat pointer index into a FatPointerId")]
pub struct FatPointerIdError;

impl TryFrom<usize> for FatPointerId {
    type Error = FatPointerIdError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        if value & !FULL_30_BITS != 0 {
            // Must fit in 30 bits (See ThinPointer)
            return Err(FatPointerIdError);
        }

        Ok(value.try_into().map(FatPointerId).unwrap()) // Do not fail
    }
}

/// Describes which pointers are set and at what index.
///
/// An inode pointer contains 32 pointers.
/// Some might be null.
///
/// Example:
/// Let's say that there are 2 pointers sets, at the index
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

    /// Return how many pointers are not null before `index`
    ///
    /// This is achieve by shifting (`32 - index` times) the bitfield to the left
    /// and counting how many bit are left
    ///
    /// Example: Consider this bitfield:
    /// `0b00000000_00001000_00010001_10000010`
    ///
    /// We call `get_index_for(8)`, we shift to the left (32 - 8 times):
    /// `0b10000010_00000000_00000000_00000000`
    ///
    /// There are now 2 bits left. This is the number of non-null pointers
    /// before the `8`th bit in the original bitfield.
    ///
    /// This is used to get the index (`ThinPointerId`) of a `ThinPointer` in
    /// `Storage::thin_pointers`.
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
/// The iterator returns the index (between 0 and 32) of the bit.
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

/// Points to a subslice of `Storage::thin_pointers`
///
/// An inode pointer may have 1 to 32 pointers.
/// Some might be null or not.
/// Each pointer, when not null, has an index associated to it (0 to 31).
///
/// - `start` is the index of the first pointer in `Storage::thin_pointers`
/// - `bitfield` is a 32 bits bitfield, that gives us how many pointers they
///   are after `start`, and at which index
///
/// Example:
/// PointersId {
///   start: `15`,
///   bitfield: `0b00000000_00001000_00010001_10000010`
/// }
///
/// This `PointersId` refers to a subslice of `Storage::thin_pointers` of length
/// 5 (there are 5 bits set in `bitfield`).
///
/// The first pointer is at `start + 0`: Storage::thin_pointers[15]
/// The second pointer is at `start + 1`: Storage::thin_pointers[16]
/// The third pointer is at `start + 2`: Storage::thin_pointers[17]
/// The 4th pointer is at `start + 3`: Storage::thin_pointers[18]
/// ...
///
/// The first pointer has the index 1
/// The second pointer has the index 7
/// The third pointer has the index 8
/// The 4th pointer has the index 12
/// ...
///
#[derive(Debug, Clone, Copy)]
pub struct PointersId {
    /// Index of first pointer in `Storage::thin_pointers`
    start: u32,
    /// A bitfield, which allow to retrieve the following pointers (after `start`)
    /// and at which index (0 to 31) they are
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

    /// Returns the `ThinPointerId` for `ptr_index`
    ///
    /// Given the `ptr_index` (which is between 0 and 31), returns
    /// where is it located in `Storage::thin_pointers`
    fn get_thin_pointer_for_ptr(&self, ptr_index: usize) -> Option<ThinPointerId> {
        let index = self.start as usize;
        let offset = self.bitfield.get_index_for(ptr_index)?;

        Some(ThinPointerId(index + offset))
    }

    pub fn iter(self) -> InodePointersIter {
        InodePointersIter::new(self)
    }
}

/// A `FatPointer` contains one of the 3 types:
/// `DirectoryId`, `HashId` or `AbsoluteOffset`
#[derive(BitfieldSpecifier)]
#[bits = 2]
#[derive(Debug, Copy, Clone)]
enum FatPointerKind {
    Directory,
    HashId,
    Offset,
}

/// A `ThinPointer` contains one of the 2 types:
/// `InodeId` or `FatPointerId`
#[derive(BitfieldSpecifier)]
#[bits = 1]
#[derive(Debug, Copy, Clone)]
enum ThinPointerKind {
    InodeId,
    FatPointer,
}

enum ThinPointerValue {
    Inode(InodeId),
    FatPointer(FatPointerId),
}

/// A `InodeId` (30 bits) or `FatPointerId` (30 bits)
///
/// This might be represented as:
///
/// ```ignore
/// struct ThinPointer {
///   is_commited: bool,
///   value: ThinPointerValue
/// }
/// ```
#[bitfield]
#[derive(Debug, Clone)]
pub struct ThinPointer {
    is_commited: bool,
    ref_kind: ThinPointerKind,
    /// Depending on `ref_kind`, this is either a `InodeId` or a `FatPointerId`
    value: B30,
}

assert_eq_size!([u8; 4], ThinPointer);

impl ThinPointer {
    fn get_value(&self) -> ThinPointerValue {
        match self.ref_kind() {
            ThinPointerKind::InodeId => ThinPointerValue::Inode(self.value().into()),
            ThinPointerKind::FatPointer => ThinPointerValue::FatPointer(self.value().into()),
        }
    }

    pub fn set_commited(&mut self, commited: bool) {
        self.set_is_commited(commited);
    }
}

/// A `DirectoryId` (8 bytes), `HashId` (6 bytes), or `AbsoluteOffset` (8 bytes).
///
/// Note that a `FatPointer` may contain a `HashId` or `AbsoluteOffset` only when
/// the pointer is deserialized from the repository.
/// When the pointer is hashed and serialized (at commit), its `HashId` and
/// `AbsoluteOffset` are stored in `Storage::pointers_data`.
///
/// This might be represented as:
///
/// ```ignore
/// struct FatPointer {
///   is_commited: bool,
///   value: DirectoryId | HashId | AbsoluteOffset
/// }
/// ```
#[bitfield]
#[derive(Clone, Copy, Debug)]
pub struct FatPointerInner {
    is_commited: bool,
    ptr_kind: FatPointerKind,
    /// This is either a:
    /// - `DirectoryId`
    /// - `HashId`
    /// - `AbsoluteOffset`
    ptr_id: B61,
}

#[derive(Clone, Debug)]
pub struct FatPointer {
    inner: Cell<FatPointerInner>,
}

assert_eq_size!([u8; 8], FatPointer);

/// A pointer on the stack, This is not stored in `Storage`
///
/// When we want to manipulate inode pointers, we fetch them
/// from `Storage::{thin,fat}_pointers` and we make them
/// a array: `[Option<PointerOnStack>; 32]`
///
/// This is used to make pointers manipulation easier
pub struct PointerOnStack {
    /// When a pointer is fetch from `Storage::thin_pointers`,
    /// this field keep its value.
    /// This is `None` when the pointer has just been deserialized from the repository
    /// or when the `FatPointer` has just been created (after an insertion with `dir_insert`)
    pub thin_pointer: Option<ThinPointer>,
    /// Value of the pointer
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
                FatPointerInner::new()
                    .with_is_commited(false)
                    .with_ptr_id(dir_or_inode_id.as_u64())
                    .with_ptr_kind(FatPointerKind::Directory),
            ),
        }
    }

    pub fn new_commited(hash_id: Option<HashId>, offset: Option<AbsoluteOffset>) -> Self {
        let (ptr_kind, ptr) = match (hash_id, offset) {
            (None, Some(offset)) => (FatPointerKind::Offset, offset.as_u64()),
            (Some(hash_id), None) => (FatPointerKind::HashId, hash_id.as_u64()),
            _ => unreachable!(
                "Self::new_commited must be call with a `HashId` or an `AbsoluteOffset`"
            ),
        };

        Self {
            inner: Cell::new(
                FatPointerInner::new()
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
                        FatPointerInner::new()
                            .with_is_commited(is_commited)
                            .with_ptr_kind(FatPointerKind::Directory)
                            .with_ptr_id(inode_id),
                    ),
                })
            }
            ThinPointerKind::FatPointer => {
                let pointer_id: FatPointerId = value.into();
                // The thin pointer points to a fat pointer, dereference it
                let fat_pointer = storage
                    .fat_pointers
                    .get(pointer_id)?
                    .cloned()
                    .ok_or(StorageError::InodePointersNotFound)?;

                Ok(fat_pointer)
            }
        }
    }

    /// Return the `DirectoryId`
    ///
    /// This returns `None` when the inner value is a `HashId` or `AbsoluteOffset`
    pub fn ptr_id(&self) -> Option<DirectoryOrInodeId> {
        let inner = self.inner.get();

        if !matches!(inner.ptr_kind(), FatPointerKind::Directory) {
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

    /// Return the `HashId` or `AbsoluteOffset`
    ///
    /// This returns `None` when the inner value is a `DirectoryId`
    pub fn get_data(&self) -> Result<Option<ObjectReference>, StorageError> {
        let inner = self.inner.get();

        let ptr_id: u64 = inner.ptr_id();

        match inner.ptr_kind() {
            FatPointerKind::Directory => Ok(None),
            FatPointerKind::HashId => {
                let hash_id = HashId::new(ptr_id).ok_or(StorageError::InvalidHashIdInPointer)?;
                Ok(Some(ObjectReference::new(Some(hash_id), None)))
            }
            FatPointerKind::Offset => {
                let offset: AbsoluteOffset = ptr_id.into();
                Ok(Some(ObjectReference::new(None, Some(offset))))
            }
        }
    }

    pub fn set_ptr_id(&self, ptr_id: DirectoryOrInodeId) {
        let mut inner = self.inner.get();

        inner.set_ptr_id(ptr_id.as_u64());
        inner.set_ptr_kind(FatPointerKind::Directory);

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

/// Index of a `ThinPointer` in `Storage::thin_pointers`
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ThinPointerId(usize);

impl From<ThinPointerId> for usize {
    fn from(value: ThinPointerId) -> Self {
        value.0
    }
}

impl From<usize> for ThinPointerId {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

/// Iterates on pointers.
///
/// This iterates on both the pointer index (0 to 31) and the `ThinPointerId`
/// (index in `Storage::thin_pointers`)
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
    type Item = (usize, ThinPointerId);

    fn next(&mut self) -> Option<Self::Item> {
        let next_ptr_index = self.bitfield_iter.next()?;
        let current = self.current;
        self.current += 1;

        let thin_pointer_id = ThinPointerId(self.start + current);
        Some((next_ptr_index, thin_pointer_id))
    }
}

#[derive(Debug)]
pub struct Inode {
    pub depth: u16,
    pub nchildren: u32,
    /// Points to a subslice of `Storage::thin_pointers`
    /// `PointersId` contains an index and a bitfield, which give
    /// details about the subslice.
    ///
    /// See `PointersId` for more information
    pub pointers: PointersId,
}

assert_eq_size!([u8; 16], Inode);

/// A `DirectoryId` or `InodeId`
///
/// When accessing `FatPointer`, its value (`FatPointer::ptr_id()`) is either
/// a `DirectoryId` or `InodeId`
///
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
    pub nodes: IndexMap<DirEntryId, DirEntry, { DEFAULT_NODES_CAPACITY }>,
    /// Concatenation of all directories in the working tree.
    /// The working tree has `DirectoryId` which refers to a subslice of this
    /// vector `directories`
    pub directories: ChunkedVec<(StringId, DirEntryId), { DEFAULT_DIRECTORIES_CAPACITY }>,
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
    blobs: ChunkedVec<u8, { DEFAULT_BLOBS_CAPACITY }>,
    /// Concatenation of all inodes.
    ///
    /// A `Inode` refers to a subslice of `Self::thin_pointers`
    ///
    /// Note that the implementation of `Storage` attempt to hide as much as
    /// possible the existence of inodes to the working tree.
    /// The working tree doesn't manipulate `InodeId` but `DirectoryId` only.
    /// A `DirectoryId` might contains an `InodeId` but it's only the root
    /// of an Inode, any children of that root are not visible to the working tree.
    ///
    /// See `PointersId` and `Inode` for more information
    inodes: IndexMap<InodeId, Inode, { DEFAULT_INODES_CAPACITY }>,
    /// Concatenation of pointers.
    /// `Self::inodes` refers to a subslice of this field.
    /// A `ThinPointer` contains either a `InodeId` (u32) or a `FatPointerId` (u32)
    ///
    /// This vector is growing very fast when manipulating inodes
    ///
    /// See `PointersId` and `Inode` for more information
    pub thin_pointers: IndexMap<ThinPointerId, ThinPointer, { DEFAULT_THIN_POINTERS_CAPACITY }>,
    /// Contains big pointers
    /// It's either a `HashId` (6 bytes), an `AbsoluteOffset` (8 bytes) or a
    /// `DirectoryId` (8 bytes)
    ///
    /// There are no duplicate in this vector.
    /// Many `ThinPointer` may refer to the same `FatPointerId`
    /// This makes the vector much smaller than `Self::thin_pointers`
    pub fat_pointers: IndexMap<FatPointerId, FatPointer, { DEFAULT_FAT_POINTERS_CAPACITY }>,
    /// Store the `ObjectReference` of the inode pointers.
    ///
    /// This is used at commit time (when inodes are hashed and serialized)
    /// When an inode is deserialized from the repository, its data (`HashId`
    /// and/or `AbsoluteOffset` are stored directly in the `FatPointer`),
    /// not in this `pointers_data`.
    ///
    /// We keep the data out of `FatPointer` and `ThinPointer` because this saves
    /// a lot of space:
    /// When inodes are modified, this creates lots of `ThinPointer` and `FatPointer`,
    /// because they are immutables.
    /// But those "intermediates" pointers do not need/have a `HashId`/`AbsoluteOffset`
    /// associated to them, they are required at commit time only.
    pointers_data: RefCell<BTreeMap<u64, ObjectReference>>,
    /// Objects bytes are read from disk into this vector
    pub data: Vec<u8>,
    /// Map of deserialized (from disk) offset to their `HashId`.
    pub offsets_to_hash_id: BTreeMap<AbsoluteOffset, HashId>,
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

assert_eq_size!([u8; 7], Option<[u8; 6]>);

impl Default for Storage {
    fn default() -> Self {
        Self::new()
    }
}

// Whether or not a non-existing key was added to the inode
type IsNewKey = bool;

const DEFAULT_DIRECTORIES_CAPACITY: usize = 512 * 1024;
const DEFAULT_BLOBS_CAPACITY: usize = 256 * 1024;
const DEFAULT_NODES_CAPACITY: usize = 128 * 1024;
const DEFAULT_INODES_CAPACITY: usize = 32 * 1024;
const DEFAULT_FAT_POINTERS_CAPACITY: usize = 32 * 1024;
const DEFAULT_THIN_POINTERS_CAPACITY: usize = 128 * 1024;

impl Storage {
    pub fn new() -> Self {
        Self {
            directories: ChunkedVec::default(), // ~4MB
            temp_dir: Vec::with_capacity(256),  // 2KB
            // Allocates `temp_inodes_index` only when used
            temp_inodes_index: Vec::new(),     // 0B
            blobs: ChunkedVec::default(),      // ~262KB
            nodes: IndexMap::default(),        // ~3MB
            inodes: IndexMap::default(),       // ~524KB
            data: Vec::with_capacity(100_000), // ~97KB
            offsets_to_hash_id: Default::default(),
            fat_pointers: IndexMap::default(), // ~262KB
            pointers_data: Default::default(),
            thin_pointers: IndexMap::default(), // ~525KB
        } // Total ~8MB
    }

    pub fn memory_usage(&self, strings: &StringInterner) -> StorageMemoryUsage {
        let nodes_cap = self.nodes.capacity();
        let directories_cap = self.directories.capacity();
        let blobs_cap = self.blobs.capacity();
        let temp_dir_cap = self.temp_dir.capacity();
        let temp_inodes_index_cap = self.temp_inodes_index.capacity();
        let inodes_cap = self.inodes.capacity();
        let thin_pointers_cap = self.thin_pointers.capacity();
        let fat_pointers_cap = self.fat_pointers.capacity();
        let pointers_data_len = self.pointers_data.borrow().len();
        let strings = strings.memory_usage();
        let total_bytes = (nodes_cap * size_of::<DirEntry>())
            .saturating_add(directories_cap * size_of::<(StringId, DirEntryId)>())
            .saturating_add(temp_dir_cap * size_of::<(StringId, DirEntryId)>())
            .saturating_add(temp_inodes_index_cap * size_of::<u8>())
            .saturating_add(fat_pointers_cap * size_of::<FatPointer>())
            .saturating_add(thin_pointers_cap * size_of::<ThinPointer>())
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
            fat_pointers_cap,
            thin_pointers_cap,
            pointers_data_len,
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

    pub fn clear(&mut self) {
        self.nodes.clear();
        self.directories.clear();
        self.blobs.clear();
        self.inodes.clear();
        self.thin_pointers.clear();
        self.fat_pointers.clear();
        if !self.pointers_data.borrow().is_empty() {
            self.pointers_data = Default::default();
        }
        if !self.offsets_to_hash_id.is_empty() {
            self.offsets_to_hash_id = Default::default();
        }
    }

    /// Set the `HashId` of `pointer` in `Self::pointers_data`
    pub fn pointer_set_hashid(
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

    /// Set the `AbsoluteOffset` of `pointer` in `Self::pointers_data`
    pub fn pointer_set_offset(
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
        let old = object_ref.offset.replace(offset);
        debug_assert!(old.is_none());

        Ok(())
    }

    /// Set the `ObjectReference` (Both `HashId` and `AbsoluteOffset`) of
    /// `pointer` in `Self::pointers_data`
    pub fn pointer_set_data(
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

    /// Returns the `AbsoluteOffset` of `pointer`
    ///
    /// This returns `None` when the pointer has not been {se/de}serialized
    pub fn pointer_retrieve_offset(
        &self,
        pointer: &FatPointer,
    ) -> Result<Option<AbsoluteOffset>, StorageError> {
        if let Some(offset) = pointer.get_data()?.and_then(|r| r.offset_opt()) {
            return Ok(Some(offset));
        }

        let ptr_id = pointer
            .ptr_id()
            .ok_or(StorageError::InodePointerIdNotFound)?
            .as_u64();

        let refs = self.pointers_data.borrow();
        let object_ref = refs
            .get(&ptr_id)
            .ok_or(StorageError::PointerDoesNotHaveData)?;

        Ok(object_ref.offset_opt())
    }

    /// Returns the `HashId` of `pointer`
    ///
    /// This returns `None` when the pointer has not been {se/de}serialized/hashed
    pub fn pointer_retrieve_hashid(
        &self,
        pointer: &FatPointer,
        repository: &ContextKeyValueStore,
    ) -> Result<Option<HashId>, HashingError> {
        let mut refs = self.pointers_data.borrow_mut();

        // The `HashId` of a `FatPointer` can be in different places:
        // - Its `ptr_id` value can be a `HashId`
        // - Its `ptr_id` value can be an offset, we take it and we
        //   use it to retrieve its `HashId` from `Storage::pointers_data`,
        //   `Storage::offsets_to_hash_id`, or by reading the repository

        let offset = match pointer.get_data()? {
            Some(object_ref) => {
                if let Some(hash_id) = object_ref.hash_id_opt() {
                    return Ok(Some(hash_id));
                }
                object_ref.offset()
            }
            None => {
                let ptr_id = pointer
                    .ptr_id()
                    .ok_or(StorageError::InodePointerIdNotFound)?
                    .as_u64();
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
    #[cfg(test)]
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

                let thin_pointer_id = match pointers.get_thin_pointer_for_ptr(index_at_depth) {
                    Some(index) => index,
                    None => return Ok(None),
                };

                let ptr_id = if let Some(ptr_id) = self.pointer_get_id(thin_pointer_id)? {
                    ptr_id
                } else {
                    self.pointer_fetch(thin_pointer_id, repository, strings)?
                };

                self.dir_find_dir_entry_recursive(ptr_id, key, strings, repository)
            }
        }
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

        let current_index: usize = current.try_into().unwrap(); // Does not fail
        if current_index & !FULL_30_BITS != 0 {
            // Must fit in 30 bits (See ThinPointer)
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

        for pointer in pointers.iter().filter_map(|p| p.as_ref()) {
            let thin_pointer: ThinPointer =
                if let Some(DirectoryOrInodeId::Inode(inode_id)) = pointer.ptr_id() {
                    // The pointer points to an `InodeId`, create a `ThinPointer`.
                    let inode_id: u32 = inode_id.into();
                    ThinPointer::new()
                        .with_ref_kind(ThinPointerKind::InodeId)
                        .with_is_commited(pointer.is_commited())
                        .with_value(inode_id)
                } else if let Some(thin_pointer) = &pointer.thin_pointer {
                    // `ThinPointer` already exist, use it.
                    // This avoid growing `Self::fat_pointers`, and it does't make duplicate `FatPointer`
                    thin_pointer.clone()
                } else {
                    // The pointers points to an `AbsoluteOffset`, `HashId` or `DirectoryId`
                    // Create a new `FatPointer`
                    let fat_pointer: FatPointerId =
                        self.fat_pointers.push(pointer.fat_pointer.clone())?;
                    let fat_pointer: u32 = fat_pointer.into();
                    ThinPointer::new()
                        .with_ref_kind(ThinPointerKind::FatPointer)
                        .with_is_commited(pointer.is_commited())
                        .with_value(fat_pointer)
                };

            self.thin_pointers.push(thin_pointer)?;
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
            // The capacity never goes above 1024, even during flattening
            self.temp_inodes_index = Vec::with_capacity(1024);
        }

        if self.temp_inodes_index.len() < dir_range.end {
            // Make sure that `Self::temp_inodes_index` is at least the same
            // size than `Self::temp_dir[dir_range]`.
            // So we can access the hash of an inode by its same index:
            // hash(Self::temp_dir[X]) = Self::temp_inodes_index[X]
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

        if dir_range_len <= INODE_POINTER_THRESHOLD {
            // The directory in `dir_range` is not guaranted to be sorted.
            // We use `Self::copy_sorted` to copy that directory into `Storage::directories` in
            // a sorted order.

            let new_dir_id = self.copy_sorted(dir_range, strings)?;

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
            }

            self.add_inode_pointers(depth, nchildren, pointers)
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

    /// Returns an array `[Option<PointerOnStack>; 32]` from `pointers`
    ///
    /// `pointers` refers to a subslice of `Self::thin_pointers`
    /// Use it to create a list of 32 `Option<PointerOnStack>`
    /// When a pointer is null, it will make a `None` in the array.
    ///
    /// See `PointersId` for more information
    pub fn into_pointers_on_stack(
        &self,
        pointers: PointersId,
    ) -> Result<[Option<PointerOnStack>; 32], StorageError> {
        let mut cloned: [Option<PointerOnStack>; 32] = Default::default();

        for (ptr_index, thin_pointer_id) in pointers.iter() {
            let thin_pointer = self
                .thin_pointers
                .get(thin_pointer_id)?
                .cloned()
                .ok_or(StorageError::ThinPointerNotFound)?;

            cloned[ptr_index] = Some(PointerOnStack {
                thin_pointer: Some(thin_pointer.clone()),
                fat_pointer: FatPointer::from_thin_pointer(thin_pointer, self)?,
            });
        }

        Ok(cloned)
    }

    /// Returns the `DirectoryOrInodeId` of the `thin_pointer_id`
    ///
    /// This returns `None` when the thin pointer points to an `AbsoluteOffset`
    /// or `HashId`
    pub fn pointer_get_id(
        &self,
        thin_pointer_id: ThinPointerId,
    ) -> Result<Option<DirectoryOrInodeId>, StorageError> {
        let thin_pointer = self
            .thin_pointers
            .get(thin_pointer_id)?
            .ok_or(StorageError::ThinPointerNotFound)?;

        match thin_pointer.get_value() {
            ThinPointerValue::Inode(inode_id) => Ok(Some(DirectoryOrInodeId::Inode(inode_id))),
            ThinPointerValue::FatPointer(fat_pointer_id) => {
                // The thin pointer points to a fat pointer, dereference it
                let fat_pointer = self
                    .fat_pointers
                    .get(fat_pointer_id)?
                    .ok_or(StorageError::FatPointerNotFound)?;
                Ok(fat_pointer.ptr_id())
            }
        }
    }

    /// Copy the `FatPointer` of `thin_pointer_id`
    ///
    /// When the thin pointer points to an `InodeId`, a new `FatPointer` is created from it
    /// When it points to a `FatPointer`, it is dereferenced and cloned
    ///
    /// Any modification made to the resulting `FatPointer` will not be reflected/updated
    /// into `Storage`, this is not a reference.
    pub fn pointer_copy(&self, thin_pointer_id: ThinPointerId) -> Result<FatPointer, StorageError> {
        let thin_pointer = self
            .thin_pointers
            .get(thin_pointer_id)?
            .ok_or(StorageError::ThinPointerNotFound)?;

        match thin_pointer.get_value() {
            ThinPointerValue::Inode(inode_id) => {
                let ptr = FatPointer::new(DirectoryOrInodeId::Inode(inode_id));
                ptr.set_commited(thin_pointer.is_commited());
                Ok(ptr)
            }
            ThinPointerValue::FatPointer(fat_pointer_id) => {
                // The thin pointer points to a fat pointer, dereference it
                self.fat_pointers
                    .get(fat_pointer_id)?
                    .cloned()
                    .ok_or(StorageError::FatPointerNotFound)
            }
        }
    }

    /// Fetch the pointer from the repository
    ///
    /// `pointer` is an `HashId` or `AbsoluteOffset` and we need to get its value
    /// from the repository.
    /// Once fetched, the `pointer` (`PointerOnStack`) is updated
    fn pointer_fetch_on_stack(
        &mut self,
        pointer: &PointerOnStack,
        repository: &ContextKeyValueStore,
        strings: &mut StringInterner,
    ) -> Result<DirectoryOrInodeId, StorageError> {
        let pointer_data = pointer
            .get_data()?
            .ok_or(StorageError::MissingDataInPointer)?;

        let pointer_inode_id = repository
            .get_inode(pointer_data, self, strings)
            .map_err(|_| StorageError::InodeInRepositoryNotFound)?;

        pointer.set_ptr_id(pointer_inode_id);

        self.pointer_set_data(pointer, pointer_data)?;

        Ok(pointer_inode_id)
    }

    /// Fetch the pointer from the repository
    ///
    /// The difference with `Self::pointer_fetch_on_stack` is that it is
    /// directly updating `Self::pointers_data` and the `FatPointer`.
    fn pointer_fetch(
        &mut self,
        thin_pointer_id: ThinPointerId,
        repository: &ContextKeyValueStore,
        strings: &mut StringInterner,
    ) -> Result<DirectoryOrInodeId, StorageError> {
        let thin_pointer = self
            .thin_pointers
            .get(thin_pointer_id)?
            .ok_or(StorageError::ThinPointerNotFound)?;

        let fat_pointer_id = match thin_pointer.get_value() {
            ThinPointerValue::Inode(_) => unreachable!(), // When `Self::pointer_fetch` is called, it's on a `FatPointer`
            ThinPointerValue::FatPointer(fat_pointer_id) => fat_pointer_id,
        };

        let pointer = self
            .fat_pointers
            .get(fat_pointer_id)?
            .ok_or(StorageError::FatPointerNotFound)?;
        let pointer_data = pointer
            .get_data()?
            .ok_or(StorageError::MissingDataInPointer)?;

        let pointer_inode_id = repository
            .get_inode(pointer_data, self, strings)
            .map_err(|_| StorageError::InodeInRepositoryNotFound)?;

        let pointer = self
            .fat_pointers
            .get(fat_pointer_id)?
            .ok_or(StorageError::FatPointerNotFound)?;

        pointer.set_ptr_id(pointer_inode_id);
        self.pointer_set_data(pointer, pointer_data)?;

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
        match ptr_id {
            DirectoryOrInodeId::Directory(dir_id) => {
                let dir_id = dir_id;
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

                pointers[index_at_depth] = Some(PointerOnStack {
                    thin_pointer: None,
                    fat_pointer: FatPointer::new(inode_id),
                });

                let nchildren = if is_new_key { nchildren + 1 } else { nchildren };

                let ptr_id = self.add_inode_pointers(depth, nchildren, pointers)?;

                Ok((ptr_id, is_new_key))
            }
        }
    }

    /// [test only] Remove hash ids in the inode and it's children
    ///
    /// This is used to force recomputing hashes
    #[cfg(test)]
    pub fn inodes_drop_hash_ids(&mut self, inode_id: InodeId) {
        let inode = self.get_inode(inode_id).unwrap();

        for (_, thin_pointer_id) in inode.pointers.iter() {
            let ptr_id = self.pointer_get_id(thin_pointer_id).unwrap().unwrap();

            self.pointers_data.borrow_mut().remove(&ptr_id.as_u64());

            if let DirectoryOrInodeId::Inode(inode_id) = ptr_id {
                self.inodes_drop_hash_ids(inode_id);
            }
        }
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

                for (_, thin_pointer_id) in inode.pointers.iter() {
                    let ptr_id = if let Some(ptr_id) = self.pointer_get_id(thin_pointer_id)? {
                        ptr_id
                    } else {
                        self.pointer_fetch(thin_pointer_id, repository, strings)?
                    };

                    self.iter_full_inodes_recursive_unsorted(ptr_id, strings, repository, fun)?;
                }
            }
        }

        Ok(())
    }

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

                for (_, thin_pointer_id) in inode.pointers.iter() {
                    // When the inode is not deserialized, ignore it
                    // See `Self::iter_full_inodes_recursive_unsorted` to iterate on
                    // the full inode
                    if let Some(ptr_id) = self.pointer_get_id(thin_pointer_id)? {
                        self.iter_inodes_recursive_unsorted(ptr_id, fun)?;
                    }
                }
            }
        }

        Ok(())
    }

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

    /// Load the full inode in `Self`
    pub fn dir_full_load(
        &mut self,
        dir_id: DirectoryId,
        strings: &mut StringInterner,
        repository: &ContextKeyValueStore,
    ) -> Result<(), MerkleError> {
        self.dir_full_iterate_unsorted(dir_id, strings, repository, |_| Ok(()))
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
            self.iter_inodes_recursive_unsorted(DirectoryOrInodeId::Inode(inode_id), &mut fun)?;
        } else {
            let dir = self.get_small_dir(dir_id)?;
            for elem in dir.as_ref() {
                fun(elem)?;
            }
        }
        Ok(())
    }

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

            return Ok(inode_id.into_dir());
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
                }
            }
            DirectoryOrInodeId::Inode(inode_id) => {
                let Inode {
                    depth,
                    nchildren,
                    pointers,
                } = self.get_inode(inode_id)?;

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
                } else {
                    let mut pointers = self.into_pointers_on_stack(*pointers)?;
                    let index_at_depth = index_of_key(*depth as u32, key) as usize;
                    let depth = *depth;

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
                        }
                        None => {
                            // The key was removed and it result in an empty directory.
                            // Remove the pointer: make it `None`.
                            pointers[index_at_depth] = None;
                        }
                    }

                    self.add_inode_pointers(depth, new_nchildren, pointers)?
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

    pub fn deallocate(&mut self) {
        self.nodes = IndexMap::empty();
        self.directories = ChunkedVec::empty();
        self.temp_dir = Vec::new();
        self.temp_inodes_index = Vec::new();
        self.blobs = ChunkedVec::empty();
        self.inodes = IndexMap::empty();
        self.data = Vec::new();
        self.offsets_to_hash_id = Default::default();
        self.thin_pointers = IndexMap::empty();
        self.fat_pointers = IndexMap::empty();
        self.pointers_data = Default::default();
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
            inner.set_is_commited(false);
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

            self.directories = ChunkedVec::default();
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
                let pointer = self.pointer_copy(ThinPointerId(index)).unwrap();

                let hash_id = match self.pointer_retrieve_hashid(&pointer, repository).unwrap() {
                    Some(hash_id) => hash_id,
                    None => continue,
                };

                let hash: ObjectHash = repository.get_hash(hash_id.into()).unwrap().into_owned();
                let new_hash_id: HashId = *unique.entry(hash).or_insert(hash_id);

                self.pointer_set_hashid(&pointer, new_hash_id)?;
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
                self.thin_pointers
                    .get_mut(ThinPointerId(i))
                    .unwrap()
                    .unwrap()
                    .forget_reference();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        kv_store::in_memory::{InMemory, InMemoryConfiguration},
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
        let repo = InMemory::try_new(InMemoryConfiguration {
            db_path: None,
            startup_check: true,
        })
        .unwrap();

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

            assert_eq!(iter.next().unwrap(), (0, ThinPointerId(start)));
            assert_eq!(iter.next().unwrap(), (1, ThinPointerId(start + 1)));
            assert_eq!(iter.next().unwrap(), (3, ThinPointerId(start + 2)));
            assert_eq!(iter.next().unwrap(), (4, ThinPointerId(start + 3)));
            assert_eq!(iter.next().unwrap(), (8, ThinPointerId(start + 4)));
            assert_eq!(iter.next().unwrap(), (30, ThinPointerId(start + 5)));
            assert_eq!(iter.next().unwrap(), (31, ThinPointerId(start + 6)));
            assert!(iter.next().is_none());
        }

        let mut bitfield = PointersBitfield::default();
        bitfield.set(5);
        bitfield.set(30);
        bitfield.set(31);

        let id = PointersId { start: 0, bitfield };

        let mut iter = InodePointersIter::new(id);

        assert_eq!(iter.next().unwrap(), (5, ThinPointerId(0)));
        assert_eq!(iter.next().unwrap(), (30, ThinPointerId(1)));
        assert_eq!(iter.next().unwrap(), (31, ThinPointerId(2)));
        assert!(iter.next().is_none());
    }
}
