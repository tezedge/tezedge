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

use serde::{Deserialize, Serialize};

use crate::hash::{hash_object, HashingError, ObjectHash};
use crate::{kv_store::HashId, ContextKeyValueStore};

use self::storage::Blob;
use self::string_interner::StringInterner;
use self::{
    storage::{BlobId, DirectoryId, Storage},
    working_tree::MerkleError,
};
use super::serialize::persistent::AbsoluteOffset;

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
    object_hash_id: B48,
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

assert_eq_size!([u8; 22], DirEntry);

/// Commit objects are the entry points to different versions of the context tree.
#[derive(Debug, Hash, Clone, Eq, PartialEq)]
pub struct Commit {
    pub parent_commit_ref: Option<ObjectReference>,
    pub root_ref: ObjectReference,
    pub time: u64,
    pub author: String,
    pub message: String,
}

impl Commit {
    pub fn set_root_offset(&mut self, offset: AbsoluteOffset) {
        self.root_ref.offset.replace(offset);
    }

    pub fn parent_hash_id(&self) -> Option<HashId> {
        self.parent_commit_ref.and_then(|p| p.hash_id_opt())
    }

    pub fn set_parent_hash_id(&mut self, hash_id: HashId) {
        match self.parent_commit_ref.as_mut() {
            Some(parent) => parent.set_hash_id(hash_id),
            None => {
                let mut obj_ref = ObjectReference::default();
                obj_ref.set_hash_id(hash_id);
                self.parent_commit_ref = Some(obj_ref);
            }
        }
    }
}

/// An object in the context repository
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Object {
    Directory(DirectoryId),
    Blob(BlobId),
    Commit(Box<Commit>),
}

#[derive(Debug, Default, Copy, Clone, Serialize, Deserialize, Hash, PartialEq, Eq)]
pub struct ObjectReference {
    hash_id: Option<HashId>,
    offset: Option<AbsoluteOffset>,
}

assert_eq_size!([u8; 24], ObjectReference);

impl From<HashId> for ObjectReference {
    fn from(hash_id: HashId) -> Self {
        Self {
            hash_id: Some(hash_id),
            offset: None,
        }
    }
}

impl ObjectReference {
    pub fn new(hash_id: Option<HashId>, offset: Option<AbsoluteOffset>) -> Self {
        Self { hash_id, offset }
    }

    pub fn set_offset(&mut self, offset: AbsoluteOffset) {
        self.offset.replace(offset);
    }

    pub fn set_hash_id(&mut self, hash_id: HashId) {
        self.hash_id.replace(hash_id);
    }

    pub fn offset(&self) -> AbsoluteOffset {
        self.offset
            .expect("ObjectReference::offset called outside of the persistent context")
    }

    pub fn offset_opt(&self) -> Option<AbsoluteOffset> {
        self.offset
    }

    pub fn hash_id(&self) -> HashId {
        self.hash_id
            .expect("ObjectReference::hash_id called outside of the in-memory context")
    }

    pub fn hash_id_opt(&self) -> Option<HashId> {
        self.hash_id
    }
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

    pub fn set_hash_id(&self, hash_id: impl Into<Option<HashId>>) {
        let hash_id: Option<HashId> = hash_id.into();
        let hash_id = hash_id.map(|h| h.as_u64()).unwrap_or(0);

        let inner = self.inner.get().with_object_hash_id(hash_id);
        self.inner.set(inner);
    }

    pub fn set_offset(&self, offset: impl Into<Option<AbsoluteOffset>>) {
        let offset = offset.into();

        if let Some(offset) = offset {
            debug_assert_ne!(offset.as_u64(), 0);
        };

        let offset = offset.unwrap_or_else(|| 0.into());
        let inner = self.inner.get().with_file_offset(offset.as_u64());

        self.inner.set(inner);

        assert_eq!(self.inner.get().file_offset(), offset.as_u64());
    }

    pub fn with_offset(self, offset: AbsoluteOffset) -> Self {
        debug_assert_ne!(offset.as_u64(), 0);

        let inner = self.inner.get().with_file_offset(offset.as_u64());

        self.inner.set(inner);
        self
    }

    pub fn get_offset(&self) -> Option<AbsoluteOffset> {
        let inner = self.inner.get();
        let offset: u64 = inner.file_offset();

        if offset != 0 {
            Some(offset.into())
        } else {
            None
        }
    }

    pub fn get_reference(&self) -> ObjectReference {
        ObjectReference::new(self.hash_id(), self.get_offset())
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

    /// Returns the `ObjectHash` of this dir_entry, computing it if necessary.
    ///
    /// If this dir_entry is an inlined blob, this will return an error.
    pub fn object_hash<'a>(
        &self,
        repository: &'a mut ContextKeyValueStore,
        storage: &Storage,
        strings: &StringInterner,
    ) -> Result<Cow<'a, ObjectHash>, HashingError> {
        let _ = self
            .object_hash_id(repository, storage, strings)?
            .ok_or(HashingError::HashIdEmpty)?;
        repository
            .get_hash(self.get_reference())
            .map_err(Into::into)
    }

    /// Returns the `HashId` of this dir_entry, it will compute the hash if necessary.
    ///
    /// If this dir_entry is an inlined blob, this will return `None`.
    fn object_hash_id(
        &self,
        repository: &mut ContextKeyValueStore,
        storage: &Storage,
        strings: &StringInterner,
    ) -> Result<Option<HashId>, HashingError> {
        match self.hash_id() {
            Some(hash_id) => Ok(Some(hash_id)),
            None => {
                match self.get_object() {
                    Some(Object::Blob(blob_id)) if blob_id.is_inline() => return Ok(None),
                    _ => {}
                }

                let hash_id = if let Some(hash_id) = self
                    .get_offset()
                    .as_ref()
                    .and_then(|off| storage.offsets_to_hash_id.get(off))
                {
                    Some(*hash_id)
                } else if self.get_object().is_none() {
                    Some(repository.get_hash_id(self.get_reference())?)
                } else {
                    hash_object(
                        self.get_object()
                            .as_ref()
                            .ok_or(HashingError::MissingObject)?,
                        repository,
                        storage,
                        strings,
                    )?
                };

                if let Some(hash_id) = hash_id {
                    let mut inner = self.inner.get();
                    inner.set_object_hash_id(hash_id.as_u64());
                    self.inner.set(inner);
                };

                Ok(hash_id)
            }
        }
    }

    pub fn get_inlined_blob<'a>(&self, storage: &'a Storage) -> Option<Blob<'a>> {
        self.get_object().and_then(|object| match object {
            Object::Blob(blob_id) if blob_id.is_inline() => storage.get_blob(blob_id).ok(),
            _ => None,
        })
    }

    pub fn is_inlined_blob(&self) -> bool {
        self.get_object()
            .map(|object| matches!(object, Object::Blob(blob_id) if blob_id.is_inline()))
            .unwrap_or(false)
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
                    .with_object_hash_id(hash_id.map(|h| h.as_u64()).unwrap_or(0))
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
