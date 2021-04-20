// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{cell::RefCell, sync::Arc};

use serde::{Deserialize, Serialize};

use crate::context::merkle::hash::EntryHash;
use crate::context::ContextValue;

use self::hash::{hash_entry, HashingError};

pub mod hash;
pub mod merkle_storage;
pub mod merkle_storage_stats;

// Tree must be an ordered structure for consistent hash in hash_tree.
// The entry names *must* be in lexicographical order, as required by the hashing algorithm.
// Currently immutable OrdMap is used to allow cloning trees without too much overhead.
pub type Tree = im::OrdMap<Arc<String>, Arc<Node>>;

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum NodeKind {
    NonLeaf,
    Leaf,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct Node {
    pub node_kind: NodeKind,
    #[serde(serialize_with = "ensure_non_null_entry_hash")]
    pub entry_hash: RefCell<Option<EntryHash>>,
    #[serde(skip)]
    pub entry: RefCell<Option<Entry>>,
}

#[derive(Debug, Hash, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Commit {
    pub(crate) parent_commit_hash: Option<EntryHash>,
    pub(crate) root_hash: EntryHash,
    pub(crate) time: u64,
    pub(crate) author: String,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum Entry {
    Tree(Tree),
    Blob(ContextValue),
    Commit(Commit),
}

impl Node {
    pub fn entry_hash(&self) -> Result<EntryHash, hash::HashingError> {
        match &mut *self
            .entry_hash
            .try_borrow_mut()
            .map_err(|_| HashingError::EntryBorrow)?
        {
            Some(hash) => Ok(*hash),
            entry_hash @ None => {
                let hash = hash_entry(
                    self.entry
                        .try_borrow()
                        .map_err(|_| HashingError::EntryBorrow)?
                        .as_ref()
                        .ok_or(HashingError::MissingEntry)?,
                )?;
                entry_hash.replace(hash);
                Ok(hash)
            }
        }
    }
}

// Make sure the node contains the entry hash when serializing
fn ensure_non_null_entry_hash<S>(
    entry_hash: &RefCell<Option<EntryHash>>,
    s: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let entry_hash_ref = entry_hash.borrow();
    let entry_hash = entry_hash_ref
        .as_ref()
        .ok_or_else(|| serde::ser::Error::custom("entry_hash missing in Node"))?;

    s.serialize_some(entry_hash)
}
