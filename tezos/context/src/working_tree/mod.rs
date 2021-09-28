// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! This module contains the implementation of the Working Tree. The Working Tree is the reification
//! of a specific version of the context tree. It can be manipulated to produce a new version that can
//! then be serialized and committed to the context repository.
//!
//! The working tree is a persistent/non-ephemeral data structure. It is immutable, and each modification
//! modifications produces a new copy of the tree (but it shares most of it's structure with the original copy).
//! As long as a reference is kept to it, older versions are still available. Discarding a new version of the tree
//! to continue working with an older version is effectively a "rollback" of the modifications.

use std::{borrow::Cow, cell::Cell};

use crate::hash::{hash_object, HashingError, ObjectHash};
use crate::{kv_store::HashId, ContextKeyValueStore};

use self::{
    storage::{BlobId, DirectoryId, Storage},
    working_tree::MerkleError,
};

pub mod serializer;
pub mod shape;
pub mod storage;
pub mod string_interner;
#[allow(clippy::module_inception)]
pub mod working_tree;

use modular_bitfield::prelude::*;
use static_assertions::assert_eq_size;

#[derive(BitfieldSpecifier)]
#[bits = 1]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
pub enum DirEntryKind {
    Blob,
    Directory,
}

#[bitfield]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
pub struct DirEntryInner {
    dir_entry_kind: DirEntryKind,
    commited: bool,
    object_hash_id: B32,
    object_available: bool,
    object_id: B61,
    file_offset: B64,
}

/// Wrapper over the children objects of a directory, containing
/// extra metadata (object kind, if it is an already commited object or a new one, etc).
/// A `DirEntry` wraps either a directory or a value/blob, never a commit, because commits
/// cannot be part of a tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirEntry {
    pub(crate) inner: Cell<DirEntryInner>,
}

assert_eq_size!([u8; 20], DirEntry);

/// Commit objects are the entry points to different versions of the context tree.
#[derive(Debug, Hash, Clone, Eq, PartialEq)]
pub struct Commit {
    pub(crate) parent_commit_hash: Option<HashId>,
    pub(crate) root_hash: HashId,
    pub(crate) root_hash_offset: u64,
    pub(crate) time: u64,
    pub(crate) author: String,
    pub(crate) message: String,
}

/// An object in the context repository
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Object {
    Directory(DirectoryId),
    Blob(BlobId),
    Commit(Box<Commit>),
}

impl DirEntry {
    pub fn is_commited(&self) -> bool {
        self.inner.get().commited()
    }

    pub fn set_commited(&self, commited: bool) {
        let inner = self.inner.get().with_commited(commited);
        self.inner.set(inner);
    }

    pub fn dir_entry_kind(&self) -> DirEntryKind {
        self.inner.get().dir_entry_kind()
    }

    /// Returns the `HashId` of this dir_entry, _without_ computing it if it doesn't exist yet.
    ///
    /// Returns `None` if the `HashId` doesn't exist.
    /// Use `Self::object_hash_id` to compute the hash.
    pub fn hash_id(&self) -> Option<HashId> {
        let id = self.inner.get().object_hash_id();
        HashId::new(id)
    }

    pub fn set_offset(&self, offset: u64) {
        let inner = self.inner.get().with_file_offset(offset);
        self.inner.set(inner);
    }

    pub fn with_offset(self, offset: u64) -> Self {
        let inner = self.inner.get().with_file_offset(offset);
        self.inner.set(inner);
        self
    }

    pub fn get_offset(&self) -> u64 {
        let inner = self.inner.get();

        // assert!(self.is_commited());

        inner.file_offset()
    }

    /// Returns the object of this `DirEntry`.
    ///
    /// It returns `None` when the object has not been fetched from the repository.
    /// In that case, `TezedgeIndex::get_object` must be used with the `HashId` of this
    /// `DirEntry` to get the object.
    /// Alternative method: `TezedgeIndex::dir_entry_object`.
    pub fn get_object(&self) -> Option<Object> {
        let inner = self.inner.get();

        if !inner.object_available() {
            return None;
        }

        let object_id: u64 = inner.object_id();
        match self.dir_entry_kind() {
            DirEntryKind::Directory => Some(Object::Directory(DirectoryId::from(object_id))),
            DirEntryKind::Blob => Some(Object::Blob(BlobId::from(object_id))),
        }
    }

    /// Returns the `ObjectHash` of this dir_entry, computing it necessary.
    ///
    /// If this dir_entry is an inlined blob, this will return an error.
    pub fn object_hash<'a>(
        &self,
        store: &'a mut ContextKeyValueStore,
        storage: &Storage,
    ) -> Result<Cow<'a, ObjectHash>, HashingError> {
        let hash_id = self
            .object_hash_id(store, storage)?
            .ok_or(HashingError::HashIdEmpty)?;
        store
            .get_hash(hash_id)?
            .ok_or(HashingError::HashIdNotFound { hash_id })
    }

    /// Returns the `HashId` of this dir_entry, it will compute the hash if necessary.
    ///
    /// If this dir_entry is an inlined blob, this will return `None`.
    pub fn object_hash_id(
        &self,
        store: &mut ContextKeyValueStore,
        storage: &Storage,
    ) -> Result<Option<HashId>, HashingError> {
        match self.hash_id() {
            Some(hash_id) => Ok(Some(hash_id)),
            None => {
                let hash_id = hash_object(
                    self.get_object()
                        .as_ref()
                        .ok_or(HashingError::MissingObject)?,
                    store,
                    storage,
                )?;
                if let Some(hash_id) = hash_id {
                    let mut inner = self.inner.get();
                    inner.set_object_hash_id(hash_id.as_u32());
                    self.inner.set(inner);
                };
                Ok(hash_id)
            }
        }
    }

    /// Constructs a `DirEntry`s wrapping an object that is new, and has not been saved
    /// to the repository yet. This objects must be saved to the repository at
    /// commit time if still reachable from the root of the working tree.
    pub fn new(dir_entry_kind: DirEntryKind, object: Object) -> Self {
        DirEntry {
            inner: Cell::new(
                DirEntryInner::new()
                    .with_commited(false)
                    .with_dir_entry_kind(dir_entry_kind)
                    .with_object_hash_id(0)
                    .with_object_available(true)
                    .with_object_id(match object {
                        Object::Directory(dir_id) => dir_id.into(),
                        Object::Blob(blob_id) => blob_id.into(),
                        Object::Commit(_) => unreachable!("A DirEntry never contains a commit"),
                    }),
            ),
        }
    }

    /// Constructs a `DirEntry`s wrapping an object that is already in the repository and must
    /// not be saved again at commit time even if still reachable from the root of the working tree.
    pub fn new_commited(
        dir_entry_kind: DirEntryKind,
        hash_id: Option<HashId>,
        object: Option<Object>,
    ) -> Self {
        DirEntry {
            inner: Cell::new(
                DirEntryInner::new()
                    .with_commited(true)
                    .with_dir_entry_kind(dir_entry_kind)
                    .with_object_hash_id(hash_id.map(|h| h.as_u32()).unwrap_or(0))
                    .with_object_available(object.is_some())
                    .with_object_id(match object {
                        Some(Object::Directory(dir_id)) => dir_id.into(),
                        Some(Object::Blob(blob_id)) => blob_id.into(),
                        Some(Object::Commit(_)) => {
                            unreachable!("A DirEntry never contains a commit")
                        }
                        None => 0,
                    }),
            ),
        }
    }

    pub fn new_directory(object: Object) -> Self {
        DirEntry::new(DirEntryKind::Directory, object)
    }

    pub fn new_blob(object: Object) -> Self {
        DirEntry::new(DirEntryKind::Blob, object)
    }

    /// Returns the `HashId` of this dir_entry, _without_ computing it if it doesn't exist yet.
    ///
    /// Returns an error if the `HashId` doesn't exist.
    /// Use `Self::object_hash_id` to compute the hash.
    pub fn get_hash_id(&self) -> Result<HashId, MerkleError> {
        self.hash_id()
            .ok_or(MerkleError::InvalidState("Missing object hash"))
    }

    /// Set the object of this dir_entry.
    pub fn set_object(&self, object: &Object) -> Result<(), MerkleError> {
        let mut inner = self.inner.get();

        match object {
            Object::Directory(dir_id) => {
                let dir_id: u64 = (*dir_id).into();
                if dir_id != inner.object_id() || !inner.object_available() {
                    inner.set_object_available(true);
                    inner.set_dir_entry_kind(DirEntryKind::Directory);
                    inner.set_object_id(dir_id);
                }
            }
            Object::Blob(blob_id) => {
                let blob_id: u64 = (*blob_id).into();
                if blob_id != inner.object_id() || !inner.object_available() {
                    inner.set_object_available(true);
                    inner.set_dir_entry_kind(DirEntryKind::Blob);
                    inner.set_object_id(blob_id);
                }
            }
            Object::Commit(_) => {
                unreachable!("A DirEntry never contains a commit")
            }
        };

        self.inner.set(inner);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::working_tree::storage::DirEntryId;
    use crate::working_tree::string_interner::StringId;

    use super::*;

    #[test]
    fn test_dir_entry() {
        assert!(!std::mem::needs_drop::<DirEntry>());
        assert!(!std::mem::needs_drop::<DirEntryId>());
        assert!(!std::mem::needs_drop::<(StringId, DirEntryId)>());
    }
}
