// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{borrow::Cow, cell::Cell};

use crate::hash::{hash_object, HashingError, ObjectHash};
use crate::{kv_store::HashId, ContextKeyValueStore};

use self::{
    storage::{BlobId, DirectoryId, Storage},
    working_tree::MerkleError,
};

pub mod serializer;
pub mod storage;
pub mod string_interner;
#[allow(clippy::module_inception)]
pub mod working_tree;
pub mod working_tree_stats; // TODO - TE-261 remove or reimplement

use modular_bitfield::prelude::*;
use static_assertions::assert_eq_size;

#[derive(BitfieldSpecifier)]
#[bits = 1]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
pub enum NodeKind {
    Leaf,
    NonLeaf,
}

#[bitfield]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
pub struct NodeInner {
    node_kind: NodeKind,
    commited: bool,
    object_hash_id: B32,
    object_available: bool,
    object_id: B61,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Node {
    pub(crate) inner: Cell<NodeInner>,
}

assert_eq_size!([u8; 12], Node);

#[derive(Debug, Hash, Clone, Eq, PartialEq)]
pub struct Commit {
    pub(crate) parent_commit_hash: Option<HashId>,
    pub(crate) root_hash: HashId,
    pub(crate) time: u64,
    pub(crate) author: String,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Object {
    Directory(DirectoryId),
    Blob(BlobId),
    Commit(Box<Commit>),
}

impl Node {
    pub fn is_commited(&self) -> bool {
        self.inner.get().commited()
    }

    pub fn set_commited(&self, commited: bool) {
        let inner = self.inner.get().with_commited(commited);
        self.inner.set(inner);
    }

    pub fn node_kind(&self) -> NodeKind {
        self.inner.get().node_kind()
    }

    /// Returns the `HashId` of this node, _without_ computing it if it doesn't exist yet.
    ///
    /// Returns `None` if the `HashId` doesn't exist.
    /// Use `Self::object_hash_id` to compute the hash.
    pub fn hash_id(&self) -> Option<HashId> {
        let id = self.inner.get().object_hash_id();
        HashId::new(id)
    }

    /// Returns the object of this `Node`.
    ///
    /// It returns `None` when the object has not been fetched from the repository.
    /// In that case, `TezedgeIndex::get_object` must be used with the `HashId` of this
    /// `Node` to get the object.
    /// Alternative method: `TezedgeIndex::node_object`.
    pub fn get_object(&self) -> Option<Object> {
        let inner = self.inner.get();

        if !inner.object_available() {
            return None;
        }

        let object_id: u64 = inner.object_id();
        match self.node_kind() {
            NodeKind::NonLeaf => Some(Object::Directory(DirectoryId::from(object_id))),
            NodeKind::Leaf => Some(Object::Blob(BlobId::from(object_id))),
        }
    }

    /// Returns the `ObjectHash` of this node, computing it necessary.
    ///
    /// If this node is an inlined blob, this will return an error.
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

    /// Returns the `HashId` of this node, it will compute the hash if necessary.
    ///
    /// If this node is an inlined blob, this will return `None`.
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

    pub fn new(node_kind: NodeKind, object: Object) -> Self {
        Node {
            inner: Cell::new(
                NodeInner::new()
                    .with_commited(false)
                    .with_node_kind(node_kind)
                    .with_object_hash_id(0)
                    .with_object_available(true)
                    .with_object_id(match object {
                        Object::Directory(dir_id) => dir_id.into(),
                        Object::Blob(blob_id) => blob_id.into(),
                        Object::Commit(_) => unreachable!("A Node never contains a commit"),
                    }),
            ),
        }
    }

    pub fn new_commited(
        node_kind: NodeKind,
        hash_id: Option<HashId>,
        object: Option<Object>,
    ) -> Self {
        Node {
            inner: Cell::new(
                NodeInner::new()
                    .with_commited(true)
                    .with_node_kind(node_kind)
                    .with_object_hash_id(hash_id.map(|h| h.as_u32()).unwrap_or(0))
                    .with_object_available(object.is_some())
                    .with_object_id(match object {
                        Some(Object::Directory(dir_id)) => dir_id.into(),
                        Some(Object::Blob(blob_id)) => blob_id.into(),
                        Some(Object::Commit(_)) => unreachable!("A Node never contains a commit"),
                        None => 0,
                    }),
            ),
        }
    }

    pub fn new_non_leaf(object: Object) -> Self {
        Node::new(NodeKind::NonLeaf, object)
    }

    pub fn new_leaf(object: Object) -> Self {
        Node::new(NodeKind::Leaf, object)
    }

    /// Returns the `HashId` of this node, _without_ computing it if it doesn't exist yet.
    ///
    /// Returns an error if the `HashId` doesn't exist.
    /// Use `Self::object_hash_id` to compute the hash.
    pub fn get_hash_id(&self) -> Result<HashId, MerkleError> {
        self.hash_id()
            .ok_or(MerkleError::InvalidState("Missing object hash"))
    }

    /// Set the object of this node.
    pub fn set_object(&self, object: &Object) -> Result<(), MerkleError> {
        let mut inner = self.inner.get();

        match object {
            Object::Directory(dir_id) => {
                let dir_id: u64 = (*dir_id).into();
                if dir_id != inner.object_id() || !inner.object_available() {
                    inner.set_object_available(true);
                    inner.set_node_kind(NodeKind::NonLeaf);
                    inner.set_object_id(dir_id);
                }
            }
            Object::Blob(blob_id) => {
                let blob_id: u64 = (*blob_id).into();
                if blob_id != inner.object_id() || !inner.object_available() {
                    inner.set_object_available(true);
                    inner.set_node_kind(NodeKind::Leaf);
                    inner.set_object_id(blob_id);
                }
            }
            Object::Commit(_) => {
                unreachable!("A Node never contains a commit")
            }
        };

        self.inner.set(inner);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::working_tree::storage::NodeId;
    use crate::working_tree::string_interner::StringId;

    use super::*;

    #[test]
    fn test_node() {
        assert!(!std::mem::needs_drop::<Node>());
        assert!(!std::mem::needs_drop::<NodeId>());
        assert!(!std::mem::needs_drop::<(StringId, NodeId)>());
    }
}
