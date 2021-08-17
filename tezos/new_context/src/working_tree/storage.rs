// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    cell::Cell,
    cmp::Ordering,
    convert::{TryFrom, TryInto},
    mem::size_of,
    ops::Range,
};

use modular_bitfield::prelude::*;
use static_assertions::assert_eq_size;
use tezos_timing::StorageMemoryUsage;

use crate::hash::index as index_of_key;
use crate::kv_store::{entries::Entries, HashId};

use super::{
    string_interner::{StringId, StringInterner},
    working_tree::MerkleError,
    Node,
};

/// Threshold when a tree must be an `Inode`
const TREE_INODE_THRESHOLD: usize = 256;

/// Threshold when a `Inode::Tree` must be converted to a another `Inode::Pointers`
const INODE_POINTER_THRESHOLD: usize = 32;

// Bitsmaks used on ids/indexes
const FULL_60_BITS: usize = 0xFFFFFFFFFFFFFFF;
const FULL_56_BITS: usize = 0xFFFFFFFFFFFFFF;
const FULL_32_BITS: usize = 0xFFFFFFFF;
const FULL_31_BITS: usize = 0x7FFFFFFF;
const FULL_28_BITS: usize = 0xFFFFFFF;
const FULL_4_BITS: usize = 0xF;

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct TreeStorageId {
    /// Note: Must fit in NodeInner.entry_id (61 bits)
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
    /// A `TreeStorageId` never contains an `InodeId` other than a root.
    /// The working tree doesn't have knowledge of inodes, it's an implementation
    /// detail of the `Storage`
    bits: u64,
}

impl Default for TreeStorageId {
    fn default() -> Self {
        Self::empty()
    }
}

impl TreeStorageId {
    fn try_new_tree(start: usize, end: usize) -> Result<Self, StorageError> {
        let length = end
            .checked_sub(start)
            .ok_or(StorageError::TreeInvalidStartEnd)?;

        if start & !FULL_32_BITS != 0 {
            // Must fit in 32 bits
            return Err(StorageError::TreeStartTooBig);
        }

        if length & !FULL_28_BITS != 0 {
            // Must fit in 28 bits
            return Err(StorageError::TreeLengthTooBig);
        }

        let tree_id = Self {
            bits: (start as u64) << 28 | length as u64,
        };

        debug_assert_eq!(tree_id.get(), (start as usize, end));

        Ok(tree_id)
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

    /// Return the length of the small tree.
    ///
    /// Use `Storage::tree_len` to get the length of all trees (including inodes)
    fn small_tree_len(self) -> usize {
        debug_assert!(!self.is_inode());

        (self.bits as usize) & FULL_28_BITS
    }

    fn get_inode_index(self) -> usize {
        debug_assert!(self.is_inode());

        self.bits as usize & FULL_60_BITS
    }

    pub fn empty() -> Self {
        // Never fails
        Self::try_new_tree(0, 0).unwrap()
    }

    pub fn is_empty(&self) -> bool {
        if self.is_inode() {
            return false;
        }

        self.small_tree_len() == 0
    }
}

impl From<TreeStorageId> for u64 {
    fn from(tree_id: TreeStorageId) -> Self {
        tree_id.bits
    }
}

impl From<u64> for TreeStorageId {
    fn from(entry_id: u64) -> Self {
        Self { bits: entry_id }
    }
}

impl From<InodeId> for TreeStorageId {
    fn from(inode_id: InodeId) -> Self {
        // Never fails, `InodeId` is 31 bits, `TreeStorageId` expects 60 bits max
        Self::try_new_inode(inode_id.0 as usize).unwrap()
    }
}

#[derive(Debug)]
pub enum StorageError {
    BlobSliceTooBig,
    BlobStartTooBig,
    BlobLengthTooBig,
    TreeInvalidStartEnd,
    TreeStartTooBig,
    TreeLengthTooBig,
    InodeIndexTooBig,
    NodeIdError,
    StringNotFound,
    TreeNotFound,
    BlobNotFound,
    NodeNotFound,
    InodeNotFound,
    ExpectingTreeGotInode,
    IterationError,
    RootOfInodeNotAPointer,
}

impl From<NodeIdError> for StorageError {
    fn from(_: NodeIdError) -> Self {
        Self::NodeIdError
    }
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct BlobStorageId {
    /// Note: Must fit in NodeInner.entry_id (61 bits)
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

impl From<BlobStorageId> for u64 {
    fn from(blob_id: BlobStorageId) -> Self {
        blob_id.bits
    }
}

impl From<u64> for BlobStorageId {
    fn from(entry: u64) -> Self {
        Self { bits: entry }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum BlobRef {
    Inline { length: u8, value: [u8; 7] },
    Ref { start: usize, end: usize },
}

impl BlobStorageId {
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
pub struct NodeId(u32);

#[derive(Debug)]
pub struct NodeIdError;

impl TryInto<usize> for NodeId {
    type Error = NodeIdError;

    fn try_into(self) -> Result<usize, Self::Error> {
        Ok(self.0 as usize)
    }
}

impl TryFrom<usize> for NodeId {
    type Error = NodeIdError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        value.try_into().map(NodeId).map_err(|_| NodeIdError)
    }
}

#[derive(Clone, Debug, Copy, PartialEq, Eq)]
pub struct InodeId(u32);

#[bitfield]
#[derive(Clone, Copy, Debug)]
pub struct PointerToInodeInner {
    hash_id: B32,
    is_commited: bool,
    inode_id: B31,
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
                    .with_hash_id(hash_id.map(|h| h.as_u32()).unwrap_or(0))
                    .with_is_commited(false)
                    .with_inode_id(inode_id.0),
            ),
        }
    }

    pub fn new_commited(hash_id: Option<HashId>, inode_id: InodeId) -> Self {
        Self {
            inner: Cell::new(
                PointerToInodeInner::new()
                    .with_hash_id(hash_id.map(|h| h.as_u32()).unwrap_or(0))
                    .with_is_commited(true)
                    .with_inode_id(inode_id.0),
            ),
        }
    }

    pub fn inode_id(&self) -> InodeId {
        let inner = self.inner.get();
        let inode_id = inner.inode_id();

        InodeId(inode_id)
    }

    pub fn hash_id(&self) -> Option<HashId> {
        let inner = self.inner.get();
        let hash_id = inner.hash_id();

        HashId::new(hash_id)
    }

    pub fn set_hash_id(&self, hash_id: Option<HashId>) {
        let mut inner = self.inner.get();
        inner.set_hash_id(hash_id.map(|h| h.as_u32()).unwrap_or(0));

        self.inner.set(inner);
    }

    pub fn is_commited(&self) -> bool {
        let inner = self.inner.get();
        inner.is_commited()
    }
}

assert_eq_size!([u8; 9], Option<PointerToInode>);

/// Inode representation used for hashing directories with > TREE_INODE_THRESHOLD entries.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug)]
pub enum Inode {
    /// Directory is a list of (StringId, NodeId)
    Directory(TreeStorageId),
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

assert_eq_size!([u8; 304], Inode);

/// A range inside `Storage::temp_tree`
type TempTreeRange = Range<usize>;

/// `Storage` contains all the data from the working tree.
///
/// This is where all trees/blobs/strings are allocated.
/// The working tree only has access to ids which refer to data inside `Storage`.
///
/// Because `Storage` is for the working tree only, it is cleared before
/// every checkout.
pub struct Storage {
    /// An efficient map `NodeId -> Node`
    nodes: Entries<NodeId, Node>,
    /// Concatenation of all trees in the working tree.
    /// The working tree has `TreeStorageId` which refers to a subslice of this
    /// vector `trees`
    trees: Vec<(StringId, NodeId)>,
    /// Temporary tree, this is used to avoid allocations when we
    /// manipulate `trees`
    /// For example, `Storage::insert` will create a new tree in `temp_tree`, once
    /// done it will copy that tree from `temp_tree` into the end of `trees`
    temp_tree: Vec<(StringId, NodeId)>,
    /// Concatenation of all blobs in the working tree.
    /// The working tree has `BlobStorageId` which refers to a subslice of this
    /// vector `blobs`.
    /// Note that blobs < 8 bytes are not included in this vector `blobs`, such
    /// blob is directly inlined in the `BlobStorageId`
    blobs: Vec<u8>,
    /// Concatenation of all strings in the working tree.
    /// The working tree has `StringId` which refers to a data inside `StringInterner`.
    strings: StringInterner,
    /// Concatenation of all inodes.
    /// Note that the implementation of `Storage` attempt to hide as much as
    /// possible the existence of inodes to the working tree.
    /// The working tree doesn't manipulate `InodeId` but `TreeStorageId` only.
    /// A `TreeStorageId` might contains an `INodeId` but it's only the root
    /// of an Inode, any children of that root are not visible to the working tree.
    inodes: Vec<Inode>,
}

#[derive(Debug)]
pub enum Blob<'a> {
    Inline { length: u8, value: [u8; 7] },
    Ref { blob: &'a [u8] },
}

impl<'a> AsRef<[u8]> for Blob<'a> {
    fn as_ref(&self) -> &[u8] {
        match self {
            Blob::Inline { length, value } => &value[..*length as usize],
            Blob::Ref { blob } => blob,
        }
    }
}

impl<'a> std::ops::Deref for Blob<'a> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

assert_eq_size!([u32; 2], (StringId, NodeId));

impl Default for Storage {
    fn default() -> Self {
        Self::new()
    }
}

// Whether or not a non-existing key was added to the inode
type IsNewKey = bool;

impl Storage {
    pub fn new() -> Self {
        Self {
            trees: Vec::with_capacity(1024),
            temp_tree: Vec::with_capacity(128),
            blobs: Vec::with_capacity(2048),
            strings: Default::default(),
            nodes: Entries::with_capacity(2048),
            inodes: Vec::with_capacity(256),
        }
    }

    pub fn memory_usage(&self) -> StorageMemoryUsage {
        let nodes_cap = self.nodes.capacity();
        let trees_cap = self.trees.capacity();
        let blobs_cap = self.blobs.capacity();
        let temp_tree_cap = self.temp_tree.capacity();
        let inodes_cap = self.inodes.capacity();
        let strings = self.strings.memory_usage();
        let total_bytes = (nodes_cap * size_of::<Node>())
            .saturating_add(trees_cap * size_of::<(StringId, NodeId)>())
            .saturating_add(temp_tree_cap * size_of::<(StringId, NodeId)>())
            .saturating_add(blobs_cap)
            .saturating_add(inodes_cap * size_of::<Inode>())
            .saturating_add(strings.total_bytes);

        StorageMemoryUsage {
            nodes_len: self.nodes.len(),
            nodes_cap,
            trees_len: self.trees.len(),
            trees_cap,
            temp_tree_cap,
            blobs_len: self.blobs.len(),
            blobs_cap,
            inodes_len: self.inodes.len(),
            inodes_cap,
            strings,
            total_bytes,
        }
    }

    pub fn get_string_id(&mut self, s: &str) -> StringId {
        self.strings.get_string_id(s)
    }

    pub fn get_str(&self, string_id: StringId) -> Result<&str, StorageError> {
        self.strings
            .get(string_id)
            .ok_or(StorageError::StringNotFound)
    }

    pub fn add_blob_by_ref(&mut self, blob: &[u8]) -> Result<BlobStorageId, StorageError> {
        // Do not consider blobs of length zero as inlined, this never
        // happens when the node is running and fix a serialization issue
        // during testing/fuzzing
        if (1..8).contains(&blob.len()) {
            BlobStorageId::try_new_inline(blob)
        } else {
            let start = self.blobs.len();
            self.blobs.extend_from_slice(blob);
            let end = self.blobs.len();

            BlobStorageId::try_new(start, end)
        }
    }

    pub fn get_blob(&self, blob_id: BlobStorageId) -> Result<Blob, StorageError> {
        match blob_id.get() {
            BlobRef::Inline { length, value } => Ok(Blob::Inline { length, value }),
            BlobRef::Ref { start, end } => {
                let blob = match self.blobs.get(start..end) {
                    Some(blob) => blob,
                    None => return Err(StorageError::BlobNotFound),
                };
                Ok(Blob::Ref { blob })
            }
        }
    }

    pub fn get_node(&self, node_id: NodeId) -> Result<&Node, StorageError> {
        self.nodes.get(node_id)?.ok_or(StorageError::NodeNotFound)
    }

    pub fn add_node(&mut self, node: Node) -> Result<NodeId, NodeIdError> {
        self.nodes.push(node).map_err(|_| NodeIdError)
    }

    /// Return the small tree `tree_id`.
    ///
    /// This returns an error when the underlying tree is an Inode.
    /// To iterate/access all trees (including inodes), `Self::tree_iterate_unsorted`
    /// and `Self::tree_to_vec_unsorted` must be used.
    pub fn get_small_tree(
        &self,
        tree_id: TreeStorageId,
    ) -> Result<&[(StringId, NodeId)], StorageError> {
        if tree_id.is_inode() {
            return Err(StorageError::ExpectingTreeGotInode);
        }

        let (start, end) = tree_id.get();
        self.trees.get(start..end).ok_or(StorageError::TreeNotFound)
    }

    /// [test only] Return the tree with owned values
    #[cfg(test)]
    pub fn get_owned_tree(&self, tree_id: TreeStorageId) -> Option<Vec<(String, Node)>> {
        let tree = self.tree_to_vec_sorted(tree_id).unwrap();

        Some(
            tree.iter()
                .flat_map(|t| {
                    let key = self.strings.get(t.0)?;
                    let node = self.nodes.get(t.1).ok()??;
                    Some((key.to_string(), node.clone()))
                })
                .collect(),
        )
    }

    fn binary_search_in_tree(
        &self,
        tree: &[(StringId, NodeId)],
        key: &str,
    ) -> Result<Result<usize, usize>, StorageError> {
        let mut error = None;

        let result = tree.binary_search_by(|value| match self.get_str(value.0) {
            Ok(value) => value.cmp(key),
            Err(e) => {
                // Take the error and stop the search
                error = Some(e);
                Ordering::Equal
            }
        });

        if let Some(e) = error {
            return Err(e);
        };

        Ok(result)
    }

    fn tree_find_node_recursive(&self, inode_id: InodeId, key: &str) -> Option<NodeId> {
        let inode = self.get_inode(inode_id).ok()?;

        match inode {
            Inode::Directory(tree_id) => self.tree_find_node(*tree_id, key),
            Inode::Pointers {
                depth, pointers, ..
            } => {
                let index_at_depth = index_of_key(*depth, key) as usize;

                let pointer = pointers.get(index_at_depth)?.as_ref()?;

                let inode_id = pointer.inode_id();
                self.tree_find_node_recursive(inode_id, key)
            }
        }
    }

    pub fn tree_find_node(&self, tree_id: TreeStorageId, key: &str) -> Option<NodeId> {
        if let Some(inode_id) = tree_id.get_inode_id() {
            self.tree_find_node_recursive(inode_id, key)
        } else {
            let tree = self.get_small_tree(tree_id).ok()?;
            let index = self.binary_search_in_tree(tree, key).ok()?.ok()?;

            Some(tree[index].1)
        }
    }

    pub fn append_to_trees(
        &mut self,
        new_tree: &mut Vec<(StringId, NodeId)>,
    ) -> Result<TreeStorageId, StorageError> {
        let start = self.trees.len();
        self.trees.append(new_tree);
        let end = self.trees.len();

        TreeStorageId::try_new_tree(start, end)
    }

    /// Use `self.temp_tree` to avoid allocations
    pub fn with_new_tree<F, R>(&mut self, fun: F) -> R
    where
        F: FnOnce(&mut Self, &mut Vec<(StringId, NodeId)>) -> R,
    {
        let mut new_tree = std::mem::take(&mut self.temp_tree);
        new_tree.clear();

        let result = fun(self, &mut new_tree);

        self.temp_tree = new_tree;
        result
    }

    pub(super) fn add_inode(&mut self, inode: Inode) -> Result<InodeId, StorageError> {
        let current = self.inodes.len();
        self.inodes.push(inode);

        if current & !FULL_31_BITS != 0 {
            // Must fit in 31 bits (See PointerToInode)
            return Err(StorageError::InodeIndexTooBig);
        }

        Ok(InodeId(current as u32))
    }

    /// Copy tree from `Self::temp_tree` into `Self::trees` in a sorted order.
    ///
    /// `tree_range` is the range of the tree in `Self::temp_tree`
    fn copy_sorted(&mut self, tree_range: TempTreeRange) -> Result<TreeStorageId, StorageError> {
        let start = self.trees.len();

        for (key_id, node_id) in &self.temp_tree[tree_range] {
            let key_str = self.get_str(*key_id)?;
            let tree = &self.trees[start..];

            match self.binary_search_in_tree(tree, key_str)? {
                Ok(found) => {
                    self.trees[start + found].1 = *node_id;
                }
                Err(index) => {
                    self.trees.insert(start + index, (*key_id, *node_id));
                }
            }
        }

        let end = self.trees.len();
        TreeStorageId::try_new_tree(start, end)
    }

    fn with_temp_tree_range<Fun>(&mut self, mut fun: Fun) -> Result<TempTreeRange, StorageError>
    where
        Fun: FnMut(&mut Self) -> Result<(), StorageError>,
    {
        let start = self.temp_tree.len();
        fun(self)?;
        let end = self.temp_tree.len();

        Ok(TempTreeRange { start, end })
    }

    fn create_inode(
        &mut self,
        depth: u32,
        tree_range: TempTreeRange,
    ) -> Result<InodeId, StorageError> {
        let tree_range_len = tree_range.end - tree_range.start;

        if tree_range_len <= INODE_POINTER_THRESHOLD {
            // The tree in `tree_range` is not guaranted to be sorted.
            // We use `Self::copy_sorted` to copy that tree into `Storage::trees` in
            // a sorted order.

            let new_tree_id = self.copy_sorted(tree_range)?;

            self.add_inode(Inode::Directory(new_tree_id))
        } else {
            let nchildren = tree_range_len as u32;
            let mut pointers: [Option<PointerToInode>; 32] = Default::default();
            let mut npointers = 0;

            for index in 0..32u8 {
                let range = self.with_temp_tree_range(|this| {
                    for i in tree_range.clone() {
                        let (key_id, node_id) = this.temp_tree[i];
                        let key = this.get_str(key_id)?;
                        if index_of_key(depth, key) as u8 == index {
                            this.temp_tree.push((key_id, node_id));
                        }
                    }
                    Ok(())
                })?;

                if range.is_empty() {
                    continue;
                }

                npointers += 1;
                let inode_id = self.create_inode(depth + 1, range)?;

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

    fn insert_tree_single_node(
        &mut self,
        key_id: StringId,
        node: Node,
    ) -> Result<TempTreeRange, StorageError> {
        let node_id = self.nodes.push(node)?;

        self.with_temp_tree_range(|this| {
            this.temp_tree.push((key_id, node_id));
            Ok(())
        })
    }

    /// Copy tree `tree_id` from `Self::trees` into `Self::temp_Tree`
    ///
    /// Note: The callers of this function expect the tree to be copied
    ///       at the end of `Self::temp_tree`. This is something to keep in mind
    ///       if the implementation of this function change
    fn copy_tree_in_temp_tree(
        &mut self,
        tree_id: TreeStorageId,
    ) -> Result<TempTreeRange, StorageError> {
        let (tree_start, tree_end) = tree_id.get();

        self.with_temp_tree_range(|this| {
            this.temp_tree
                .extend_from_slice(&this.trees[tree_start..tree_end]);
            Ok(())
        })
    }

    fn insert_inode(
        &mut self,
        depth: u32,
        inode_id: InodeId,
        key: &str,
        key_id: StringId,
        node: Node,
    ) -> Result<(InodeId, IsNewKey), StorageError> {
        let inode = self.get_inode(inode_id)?;

        match inode {
            Inode::Directory(tree_id) => {
                let tree_id = *tree_id;
                let node_id = self.add_node(node)?;

                // Copy the existing tree into `Self::temp_tree` to create an inode
                let range = self.with_temp_tree_range(|this| {
                    let range = this.copy_tree_in_temp_tree(tree_id)?;

                    // We're using `Vec::insert` below and we don't want to invalidate
                    // any existing `TempTreeRange`
                    debug_assert_eq!(range.end, this.temp_tree.len());

                    let start = range.start;
                    match this.binary_search_in_tree(&this.temp_tree[range], key)? {
                        Ok(found) => this.temp_tree[start + found] = (key_id, node_id),
                        Err(index) => this.temp_tree.insert(start + index, (key_id, node_id)),
                    }

                    Ok(())
                })?;

                let new_inode_id = self.create_inode(depth, range)?;
                let is_new_key = self.inode_len(new_inode_id)? != tree_id.small_tree_len();

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
                    self.insert_inode(depth + 1, inode_id, key, key_id, node)?
                } else {
                    npointers += 1;

                    let new_tree_id = self.insert_tree_single_node(key_id, node)?;
                    let inode_id = self.create_inode(depth, new_tree_id)?;
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
        Fun: FnMut(&(StringId, NodeId)) -> Result<(), MerkleError>,
    {
        match inode {
            Inode::Pointers { pointers, .. } => {
                for pointer in pointers.iter().filter_map(|p| p.as_ref()) {
                    let inode_id = pointer.inode_id();
                    let inode = self.get_inode(inode_id)?;
                    self.iter_inodes_recursive_unsorted(inode, fun)?;
                }
            }
            Inode::Directory(tree_id) => {
                let tree = self.get_small_tree(*tree_id)?;
                for elem in tree {
                    fun(elem)?;
                }
            }
        };

        Ok(())
    }

    /// Iterate on `tree_id`.
    ///
    /// The elements won't be sorted when the underlying tree is an `Inode`.
    /// `Self::tree_to_vec_sorted` can be used to get the tree sorted.
    pub fn tree_iterate_unsorted<Fun>(
        &self,
        tree_id: TreeStorageId,
        mut fun: Fun,
    ) -> Result<(), MerkleError>
    where
        Fun: FnMut(&(StringId, NodeId)) -> Result<(), MerkleError>,
    {
        if let Some(inode_id) = tree_id.get_inode_id() {
            let inode = self.get_inode(inode_id)?;

            self.iter_inodes_recursive_unsorted(inode, &mut fun)?;
        } else {
            let tree = self.get_small_tree(tree_id)?;
            for elem in tree {
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
            Inode::Directory(tree_id) => Ok(tree_id.small_tree_len()),
        }
    }

    /// Return the number of nodes in `tree_id`.
    pub fn tree_len(&self, tree_id: TreeStorageId) -> Result<usize, StorageError> {
        if let Some(inode_id) = tree_id.get_inode_id() {
            self.inode_len(inode_id)
        } else {
            Ok(tree_id.small_tree_len())
        }
    }

    /// Make a vector of `(StringId, NodeId)`
    ///
    /// The vector won't be sorted when the underlying tree is an Inode.
    /// `Self::tree_to_vec_sorted` can be used to get the vector sorted.
    pub fn tree_to_vec_unsorted(
        &self,
        tree_id: TreeStorageId,
    ) -> Result<Vec<(StringId, NodeId)>, MerkleError> {
        let mut vec = Vec::with_capacity(self.tree_len(tree_id)?);

        self.tree_iterate_unsorted(tree_id, |&(key_id, node_id)| {
            vec.push((key_id, node_id));
            Ok(())
        })?;

        Ok(vec)
    }

    /// Make a vector of `(StringId, NodeId)` sorted by the key
    ///
    /// This is an expensive method when the underlying tree is an Inode,
    /// `Self::tree_to_vec_unsorted` should be used when the ordering is
    /// not important
    pub fn tree_to_vec_sorted(
        &self,
        tree_id: TreeStorageId,
    ) -> Result<Vec<(StringId, NodeId)>, MerkleError> {
        if tree_id.get_inode_id().is_some() {
            let mut tree = self.tree_to_vec_unsorted(tree_id)?;

            let mut error = None;

            tree.sort_unstable_by(|a, b| {
                let a = match self.get_str(a.0) {
                    Ok(a) => a,
                    Err(e) => {
                        error = Some(e);
                        ""
                    }
                };

                let b = match self.get_str(b.0) {
                    Ok(b) => b,
                    Err(e) => {
                        error = Some(e);
                        ""
                    }
                };

                a.cmp(b)
            });

            if let Some(e) = error {
                return Err(e.into());
            };

            Ok(tree)
        } else {
            let tree = self.get_small_tree(tree_id)?;
            Ok(tree.to_vec())
        }
    }

    pub fn get_inode(&self, inode_id: InodeId) -> Result<&Inode, StorageError> {
        self.inodes
            .get(inode_id.0 as usize)
            .ok_or(StorageError::InodeNotFound)
    }

    pub fn tree_insert(
        &mut self,
        tree_id: TreeStorageId,
        key_str: &str,
        node: Node,
    ) -> Result<TreeStorageId, StorageError> {
        let key_id = self.get_string_id(key_str);

        // Are we inserting in an Inode ?
        if let Some(inode_id) = tree_id.get_inode_id() {
            let (inode_id, _) = self.insert_inode(0, inode_id, key_str, key_id, node)?;
            self.temp_tree.clear();
            return Ok(inode_id.into());
        }

        let node_id = self.nodes.push(node)?;

        let tree_id = self.with_new_tree(|this, new_tree| {
            let tree = this.get_small_tree(tree_id)?;

            let index = this.binary_search_in_tree(tree, key_str)?;

            match index {
                Ok(found) => {
                    new_tree.extend_from_slice(tree);
                    new_tree[found].1 = node_id;
                }
                Err(index) => {
                    new_tree.extend_from_slice(&tree[..index]);
                    new_tree.push((key_id, node_id));
                    new_tree.extend_from_slice(&tree[index..]);
                }
            }

            this.append_to_trees(new_tree)
        })?;

        // We only check at the end of this function if the new tree length
        // is > TREE_INODE_THRESHOLD because inserting an element in a tree of length
        // TREE_INODE_THRESHOLD doesn't necessary mean that the resulting tree will
        // be bigger (if the key already exist).
        let tree_len = tree_id.small_tree_len();

        if tree_len <= TREE_INODE_THRESHOLD {
            Ok(tree_id)
        } else {
            // Copy the new tree in `Self::temp_tree`.
            let range = self.copy_tree_in_temp_tree(tree_id)?;
            // Remove the newly created tree from `Self::trees` to save memory.
            // It won't be used anymore as we're creating an inode.
            self.trees.truncate(self.trees.len() - tree_len);

            let inode_id = self.create_inode(0, range)?;
            self.temp_tree.clear();

            Ok(inode_id.into())
        }
    }

    fn remove_in_inode_recursive(
        &mut self,
        inode_id: InodeId,
        key: &str,
    ) -> Result<Option<InodeId>, StorageError> {
        let inode = self.get_inode(inode_id)?;

        match inode {
            Inode::Directory(tree_id) => {
                let tree_id = *tree_id;
                let new_tree_id = self.tree_remove(tree_id, key)?;

                if new_tree_id.is_empty() {
                    // The tree is now empty, return None to indicate that it
                    // should be removed from the Inode::Pointers
                    Ok(None)
                } else if new_tree_id == tree_id {
                    // The key was not found in the tree, so it's the same tree.
                    // Do not create a new inode.
                    Ok(Some(inode_id))
                } else {
                    self.add_inode(Inode::Directory(new_tree_id)).map(Some)
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

                    let tree_id = self.inodes_to_tree_sorted(inode_id)?;
                    let new_tree_id = self.tree_remove(tree_id, key)?;

                    if tree_id == new_tree_id {
                        // The key was not found.
                        return Ok(Some(inode_id));
                    }

                    self.add_inode(Inode::Directory(new_tree_id))?
                } else {
                    let index_at_depth = index_of_key(depth, key) as usize;

                    let pointer = match pointers[index_at_depth].as_ref() {
                        Some(pointer) => pointer,
                        None => return Ok(Some(inode_id)), // The key was not found
                    };

                    let ptr_inode_id = pointer.inode_id();
                    let mut pointers = pointers.clone();

                    let new_ptr_inode_id = self.remove_in_inode_recursive(ptr_inode_id, key)?;

                    if let Some(new_ptr_inode_id) = new_ptr_inode_id {
                        if new_ptr_inode_id == ptr_inode_id {
                            // The key was not found, don't create a new inode
                            return Ok(Some(inode_id));
                        }

                        pointers[index_at_depth] =
                            Some(PointerToInode::new(None, new_ptr_inode_id));
                    } else {
                        // The key was removed and it result in an empty directory.
                        // Remove the pointer: make it `None`.
                        pointers[index_at_depth] = None;
                        npointers -= 1;
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

    /// Convert the Inode into a small tree
    ///
    /// This traverses all elements (all children) of this `inode_id` and
    /// copy them into `Self::trees` in a sorted order.
    fn inodes_to_tree_sorted(&mut self, inode_id: InodeId) -> Result<TreeStorageId, StorageError> {
        // Iterator on the inodes children and copy all nodes into `Self::temp_tree`
        self.with_new_tree::<_, Result<_, StorageError>>(|this, temp_tree| {
            let inode = this.get_inode(inode_id)?;

            this.iter_inodes_recursive_unsorted(inode, &mut |value| {
                temp_tree.push(*value);
                Ok(())
            })
            .map_err(|_| StorageError::IterationError)?;

            Ok(())
        })?;

        // Copy nodes from `Self::temp_tree` into `Self::trees` sorted
        self.copy_sorted(TempTreeRange {
            start: 0,
            end: self.temp_tree.len(),
        })
    }

    fn remove_in_inode(
        &mut self,
        inode_id: InodeId,
        key: &str,
    ) -> Result<TreeStorageId, StorageError> {
        let inode_id = self.remove_in_inode_recursive(inode_id, key)?;
        let inode_id = inode_id.ok_or(StorageError::RootOfInodeNotAPointer)?;

        if self.inode_len(inode_id)? > TREE_INODE_THRESHOLD {
            Ok(inode_id.into())
        } else {
            // There is now TREE_INODE_THRESHOLD or less items:
            // Convert the inode into a 'small' tree
            self.inodes_to_tree_sorted(inode_id)
        }
    }

    pub fn tree_remove(
        &mut self,
        tree_id: TreeStorageId,
        key: &str,
    ) -> Result<TreeStorageId, StorageError> {
        if let Some(inode_id) = tree_id.get_inode_id() {
            return self.remove_in_inode(inode_id, key);
        };

        self.with_new_tree(|this, new_tree| {
            let tree = this.get_small_tree(tree_id)?;

            if tree.is_empty() {
                return Ok(tree_id);
            }

            let index = match this.binary_search_in_tree(tree, key)? {
                Ok(index) => index,
                Err(_) => return Ok(tree_id),
            };

            if index > 0 {
                new_tree.extend_from_slice(&tree[..index]);
            }
            if index + 1 != tree.len() {
                new_tree.extend_from_slice(&tree[index + 1..]);
            }

            this.append_to_trees(new_tree)
        })
    }

    pub fn clear(&mut self) {
        self.strings.clear();

        if self.blobs.capacity() > 2048 {
            self.blobs = Vec::with_capacity(2048);
        } else {
            self.blobs.clear();
        }

        if self.nodes.capacity() > 4096 {
            self.nodes = Entries::with_capacity(4096);
        } else {
            self.nodes.clear();
        }

        if self.trees.capacity() > 16384 {
            self.trees = Vec::with_capacity(16384);
        } else {
            self.trees.clear();
        }

        if self.inodes.capacity() > 256 {
            self.inodes = Vec::with_capacity(256);
        } else {
            self.inodes.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::working_tree::{Entry, NodeKind::Leaf};

    use super::*;

    #[test]
    fn test_storage() {
        let mut storage = Storage::new();

        let blob_id = storage.add_blob_by_ref(&[1]).unwrap();
        let entry = Entry::Blob(blob_id);

        let blob2_id = storage.add_blob_by_ref(&[2]).unwrap();
        let entry2 = Entry::Blob(blob2_id);

        let node1 = Node::new(Leaf, entry.clone());
        let node2 = Node::new(Leaf, entry2.clone());

        let tree_id = TreeStorageId::empty();
        let tree_id = storage.tree_insert(tree_id, "a", node1.clone()).unwrap();
        let tree_id = storage.tree_insert(tree_id, "b", node2.clone()).unwrap();
        let tree_id = storage.tree_insert(tree_id, "0", node1.clone()).unwrap();

        assert_eq!(
            storage.get_owned_tree(tree_id).unwrap(),
            &[
                ("0".to_string(), node1.clone()),
                ("a".to_string(), node1.clone()),
                ("b".to_string(), node2.clone()),
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
