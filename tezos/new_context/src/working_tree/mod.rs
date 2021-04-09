// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::hash::EntryHash;
use crate::ContextValue;

pub mod working_tree;
pub mod working_tree_stats;

// Tree must be an ordered structure for consistent hash in hash_tree.
// The entry names *must* be in lexicographical order, as required by the hashing algorithm.
// Currently immutable OrdMap is used to allow cloning trees without too much overhead.
pub type Tree = im::OrdMap<String, Arc<Node>>;

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum NodeKind {
    NonLeaf,
    Leaf,
}

// TODO: the value is serialized like this,
// but it is not the most convenient representation for working with the tree
// because it requires a hashmap to be able to retrieve objects by hash.
// If nodes contain inline values, serialization must not be direct but instead
// conversion into this representation must happen first.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct Node {
    pub node_kind: NodeKind,
    pub entry_hash: Arc<EntryHash>,
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
