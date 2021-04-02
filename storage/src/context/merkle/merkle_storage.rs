// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! # MerkleStorage
//!
//! Abstract merkle storage with git-like semantics and history which can be used with different K-V stores.
//!
//! # Data Structure
//! A storage with just one key `a/b/c` and its corresponding value `8` is represented like this:
//!
//! ``
//! [commit] ---> [tree1] --a--> [tree2] --b--> [tree3] --c--> [blob_8]
//! ``
//!
//! The db then contains the following:
//! ```no_compile
//! <hash_of_blob; blob8>
//! <hash_of_tree3, tree3>, where tree3 is a map {c: hash_blob8}
//! <hash_of_tree2, tree2>, where tree2 is a map {b: hash_of_tree3}
//! <hash_of_tree2, tree2>, where tree1 is a map {a: hash_of_tree2}
//! <hash_of_commit>; commit>, where commit points to the root tree (tree1)
//! ```
//!
//! Then, when looking for a path a/b/c in a spcific commit, we first get the hash of the root tree
//! from the commit, then get the tree from the database, get the hash of "a", look it up in the db,
//! get the hash of "b" from that tree, load from db, then get the hash of "c" and retrieve the
//! final value.
//!
//!
//! Now, let's assume we want to add a path `X` also referencing the value `8`. That creates a new
//! tree that reuses the previous subtree for `a/b/c` and branches away from root for `X`:
//!
//! ```no_compile
//! [tree1] --a--> [tree2] --b--> [tree3] --c--> [blob_8]
//!                   ^                             ^
//!                   |                             |
//! [tree_X]----a-----                              |
//!     |                                           |
//!      ----------------------X--------------------
//! ```
//!
//! The following is added to the database:
//! ``
//! <hash_of_tree_X; tree_X>, where tree_X is a map {a: hash_of_tree2, X: hash_of_blob8}
//! ``
//!
//! Reference: https://git-scm.com/book/en/v2/Git-Internals-Git-Objects
use std::{array::TryFromSliceError, sync::Arc};
use std::{cell::RefCell, collections::HashMap};

use failure::{Error, Fail};
use serde::Deserialize;
use serde::Serialize;

use crypto::hash::{FromBytesError, HashType};

use crate::context::gc::GarbageCollectionError;
use crate::context::merkle::hash::EntryHash;
use crate::context::merkle::hash::{hash_commit, hash_entry, hash_tree, HashingError};
use crate::context::merkle::merkle_storage_stats::{
    MerkleStorageAction, MerkleStoragePerfReport, MerkleStorageStatistics, StatUpdater,
};
use crate::context::merkle::{Commit, Entry, Node, NodeKind, Tree};
use crate::context::{
    ContextKey, ContextKeyValueStore, ContextValue, StringTreeEntry, StringTreeMap, TreeId,
};
use crate::persistent;
use crate::persistent::Flushable;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SetAction {
    key: ContextKey,
    value: ContextValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CopyAction {
    from_key: ContextKey,
    to_key: ContextKey,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RemoveAction {
    key: ContextKey,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum Action {
    Set(SetAction),
    Copy(CopyAction),
    Remove(RemoveAction),
}

pub struct MerkleStorage {
    /// Current working tree (currently checked out context)
    working_tree: (Tree, TreeId),
    /// key value storage backend
    db: Box<ContextKeyValueStore>,
    /// all different versions of the working tree
    trees: HashMap<TreeId, Tree>,
    /// Last commit hash
    last_commit_hash: Option<EntryHash>,
    /// storage latency statistics
    stats: MerkleStorageStatistics,
}

unsafe impl Send for MerkleStorage {}

#[derive(Debug, Fail)]
pub enum MerkleError {
    /// External libs errors
    #[fail(display = "RocksDB error: {:?}", error)]
    DBError {
        error: persistent::database::DBError,
    },
    #[fail(display = "KVStore error: {:?}", error)]
    KVBackendError { error: GarbageCollectionError },
    #[fail(display = "Serialization error: {:?}", error)]
    SerializationError { error: bincode::Error },
    #[fail(display = "Backend error: {:?}", error)]
    GarbageCollectionError { error: GarbageCollectionError },

    /// Internal unrecoverable bugs that should never occur
    #[fail(display = "No root retrieved for this commit!")]
    CommitRootNotFound,
    #[fail(display = "Cannot commit without a predecessor!")]
    MissingAncestorCommit,
    #[fail(
        display = "There is a commit or three under key {:?}, but not a value!",
        key
    )]
    ValueIsNotABlob { key: String },
    #[fail(
        display = "There is a blob or commit under key {:?}, but not a tree!",
        key
    )]
    ValueIsNotATree { key: String },
    #[fail(
        display = "Found wrong structure. Was looking for {}, but found {}",
        sought, found
    )]
    FoundUnexpectedStructure { sought: String, found: String },
    #[fail(display = "Entry not found! Hash={}", hash)]
    EntryNotFound { hash: String },
    #[fail(display = "Tree not in working tree! TreeId={}", tree_id)]
    TreeNotFoundInWorkingTree { tree_id: TreeId },

    /// Wrong user input errors
    #[fail(display = "No value under key {:?}.", key)]
    ValueNotFound { key: String },
    #[fail(display = "Cannot search for an empty key.")]
    KeyEmpty,
    #[fail(display = "Failed to convert hash into array: {}", error)]
    HashToArrayError { error: TryFromSliceError },
    #[fail(display = "Failed to convert hash into string: {}", error)]
    HashToStringError { error: FromBytesError },
    #[fail(display = "Failed to encode hash: {}", error)]
    HashingError { error: HashingError },
    #[fail(display = "Expected value instead of `None` for {}", _0)]
    ValueExpected(&'static str),
    #[fail(display = "Invalid state: {}", _0)]
    InvalidState(&'static str),
    #[fail(display = "GC was called before first commit")]
    GCTriggeredBeforeFirstCommit,
}

impl From<persistent::database::DBError> for MerkleError {
    fn from(error: persistent::database::DBError) -> Self {
        MerkleError::DBError { error }
    }
}

impl From<HashingError> for MerkleError {
    fn from(error: HashingError) -> Self {
        Self::HashingError { error }
    }
}

impl From<GarbageCollectionError> for MerkleError {
    fn from(error: GarbageCollectionError) -> Self {
        Self::GarbageCollectionError { error }
    }
}

impl From<bincode::Error> for MerkleError {
    fn from(error: bincode::Error) -> Self {
        Self::SerializationError { error }
    }
}

impl From<TryFromSliceError> for MerkleError {
    fn from(error: TryFromSliceError) -> Self {
        Self::HashToArrayError { error }
    }
}

impl From<FromBytesError> for MerkleError {
    fn from(error: FromBytesError) -> Self {
        Self::HashToStringError { error }
    }
}

#[derive(Debug, Fail)]
pub enum CheckEntryHashError {
    #[fail(display = "MerkleError error: {:?}", error)]
    MerkleError { error: MerkleError },
    #[fail(
        display = "Calculated hash for {} not matching expected hash: expected {:?}, calculated {:?}",
        entry_type, calculated, expected
    )]
    InvalidHashError {
        entry_type: String,
        calculated: String,
        expected: String,
    },
}

impl MerkleStorage {
    pub fn new(db: Box<ContextKeyValueStore>) -> Self {
        let tree = Tree::new();
        let tree_id = 0;
        let mut trees_map: HashMap<TreeId, Tree> = HashMap::new();

        trees_map.insert(tree_id, tree.clone());

        MerkleStorage {
            db,
            trees: trees_map,
            working_tree: (tree, tree_id),
            last_commit_hash: None,
            stats: MerkleStorageStatistics::default(),
        }
    }

    /// Get value from current working tree
    pub fn get(&mut self, key: &ContextKey) -> Result<ContextValue, MerkleError> {
        let stat_updater = StatUpdater::new(MerkleStorageAction::Get, Some(key));

        let root = self.get_working_tree_root_ref();
        let rv = self.get_from_tree(root, key).or_else(|_| Ok(Vec::new()));

        stat_updater.update_execution_stats(&mut self.stats);
        rv
    }

    /// Check if value exists in current working tree
    pub fn mem(&mut self, key: &ContextKey) -> Result<bool, MerkleError> {
        let stat_updater = StatUpdater::new(MerkleStorageAction::Mem, Some(key));

        let root = self.get_working_tree_root_ref();
        let rv = self.value_exists(root, key);

        stat_updater.update_execution_stats(&mut self.stats);
        rv
    }

    /// Check if directory exists in current working tree
    pub fn dirmem(&mut self, key: &ContextKey) -> Result<bool, MerkleError> {
        let stat_updater = StatUpdater::new(MerkleStorageAction::DirMem, Some(key));

        let root = self.get_working_tree_root_ref();
        let rv = self.directory_exists(root, key);

        stat_updater.update_execution_stats(&mut self.stats);
        Ok(rv)
    }

    /// Get value. Working tree is checked first, then last (checked out) commit.
    pub fn get_by_prefix(
        &mut self,
        prefix: &ContextKey,
    ) -> Result<Option<Vec<(ContextKey, ContextValue)>>, MerkleError> {
        let root = self.get_working_tree_root_ref();
        self._get_key_values_by_prefix(root, prefix)
    }

    /// Get value from historical context identified by commit hash.
    pub fn get_history(
        &mut self,
        commit_hash: &EntryHash,
        key: &ContextKey,
    ) -> Result<ContextValue, MerkleError> {
        let stat_updater = StatUpdater::new(MerkleStorageAction::GetHistory, Some(key));

        let commit = self.get_commit(commit_hash)?;
        let entry = self.get_entry_from_hash(&commit.root_hash)?;
        let rv = self.get_from_tree(self.get_tree(&entry)?, key);

        stat_updater.update_execution_stats(&mut self.stats);
        rv
    }

    fn value_exists(&self, tree: &Tree, key: &ContextKey) -> Result<bool, MerkleError> {
        let file = key.last().ok_or(MerkleError::KeyEmpty)?;
        let path = &key[..key.len() - 1];

        // find tree by path
        self.find_tree(&tree, &path)
            .map(|node| node.get(file).is_some())
            .or(Ok(false))
    }

    fn directory_exists(&self, root: &Tree, key: &ContextKey) -> bool {
        // find tree by path
        self.find_tree(root, &key)
            .map(|node| !node.is_empty())
            .unwrap_or(false)
    }

    fn get_from_tree(&self, root: &Tree, key: &ContextKey) -> Result<ContextValue, MerkleError> {
        let file = key.last().ok_or(MerkleError::KeyEmpty)?;
        let path = &key[..key.len() - 1];

        // find tree by path
        let node = self.find_tree(&root, &path)?;

        // get file node from tree
        let node = match node.get(file) {
            None => {
                return Err(MerkleError::ValueNotFound {
                    key: self.key_to_string(key),
                });
            }
            Some(entry) => entry,
        };
        // get blob by hash
        match self.get_entry(&node)? {
            Entry::Blob(blob) => Ok(blob),
            _ => Err(MerkleError::ValueIsNotABlob {
                key: self.key_to_string(key),
            }),
        }
    }

    // TODO: recursion is risky (stack overflow) and inefficient, try to do it iteratively..
    fn get_key_values_from_tree_recursively(
        &self,
        path: &str,
        entry: &Entry,
        entries: &mut Vec<(ContextKey, ContextValue)>,
    ) -> Result<(), MerkleError> {
        match entry {
            Entry::Blob(blob) => {
                // push key-value pair
                entries.push((self.string_to_key(path), blob.clone()));
                Ok(())
            }
            Entry::Tree(tree) => {
                // Go through all descendants and gather errors. Remap error if there is a failure
                // anywhere in the recursion paths. TODO: is revert possible?
                tree.iter()
                    .map(|(key, child_node)| {
                        let fullpath = path.to_owned() + "/" + key;

                        match self.get_entry(&child_node) {
                            Err(_) => Ok(()),
                            Ok(entry) => self
                                .get_key_values_from_tree_recursively(&fullpath, &entry, entries),
                        }
                    })
                    .find_map(|res| match res {
                        Ok(_) => None,
                        Err(err) => Some(Err(err)),
                    })
                    .unwrap_or(Ok(()))
            }
            Entry::Commit(commit) => match self.get_entry_from_hash(&commit.root_hash) {
                Err(err) => Err(err),
                Ok(entry) => self.get_key_values_from_tree_recursively(path, &entry, entries),
            },
        }
    }

    /// Go recursively down the tree from Entry, build string tree and return it
    /// (or return hex value if Blob)
    fn get_context_recursive(
        &self,
        path: &str,
        entry: &Entry,
        depth: Option<usize>,
    ) -> Result<StringTreeEntry, MerkleError> {
        if let Some(0) = depth {
            return Ok(StringTreeEntry::Null);
        }

        match entry {
            Entry::Blob(blob) => Ok(StringTreeEntry::Blob(hex::encode(blob))),
            Entry::Tree(tree) => {
                // Go through all descendants and gather errors. Remap error if there is a failure
                // anywhere in the recursion paths. TODO: is revert possible?
                let mut new_tree = StringTreeMap::new();
                for (key, child_node) in tree.iter() {
                    let fullpath = path.to_owned() + "/" + key;
                    let entry = self.get_entry(&child_node)?;
                    let rdepth = depth.map(|d| d - 1);
                    new_tree.insert(
                        key.to_string(),
                        self.get_context_recursive(&fullpath, &entry, rdepth)?,
                    );
                }
                Ok(StringTreeEntry::Tree(new_tree))
            }
            Entry::Commit(_) => Err(MerkleError::FoundUnexpectedStructure {
                sought: "Tree/Blob".to_string(),
                found: "Commit".to_string(),
            }),
        }
    }

    /// Get context tree under given prefix in string form (for JSON)
    /// depth - None returns full tree
    pub fn get_context_tree_by_prefix(
        &mut self,
        context_hash: &EntryHash,
        prefix: &ContextKey,
        depth: Option<usize>,
    ) -> Result<StringTreeEntry, MerkleError> {
        if let Some(0) = depth {
            return Ok(StringTreeEntry::Null);
        }

        let stat_updater =
            StatUpdater::new(MerkleStorageAction::GetContextTreeByPrefix, Some(prefix));
        let mut out = StringTreeMap::new();
        let commit = self.get_commit(context_hash)?;
        let entry = self.get_entry_from_hash(&commit.root_hash)?;
        let root_tree = self.get_tree(&entry)?;
        let prefixed_tree = self.find_tree(root_tree, prefix)?;

        for (key, child_node) in prefixed_tree.iter() {
            let entry = self.get_entry(&child_node)?;

            let delimiter = if prefix.is_empty() { "" } else { "/" };

            // construct full path as Tree key is only one chunk of it
            let fullpath = self.key_to_string(prefix) + delimiter + key;
            let rdepth = depth.map(|d| d - 1);
            out.insert(
                key.to_string(),
                self.get_context_recursive(&fullpath, &entry, rdepth)?,
            );
        }

        stat_updater.update_execution_stats(&mut self.stats);
        Ok(StringTreeEntry::Tree(out))
    }

    /// Construct Vec of all context key-values under given prefix
    pub fn get_key_values_by_prefix(
        &mut self,
        context_hash: &EntryHash,
        prefix: &ContextKey,
    ) -> Result<Option<Vec<(ContextKey, ContextValue)>>, MerkleError> {
        let stat_updater =
            StatUpdater::new(MerkleStorageAction::GetKeyValuesByPrefix, Some(prefix));

        let commit = self.get_commit(context_hash)?;
        let entry = self.get_entry_from_hash(&commit.root_hash)?;
        let root_tree = self.get_tree(&entry)?;
        let rv = self._get_key_values_by_prefix(root_tree, prefix);

        stat_updater.update_execution_stats(&mut self.stats);
        rv
    }

    fn _get_key_values_by_prefix(
        &self,
        root_tree: &Tree,
        prefix: &ContextKey,
    ) -> Result<Option<Vec<(ContextKey, ContextValue)>>, MerkleError> {
        let prefixed_tree = self.find_tree(root_tree, prefix)?;
        let mut keyvalues: Vec<(ContextKey, ContextValue)> = Vec::new();

        for (key, child_node) in prefixed_tree.iter() {
            let entry = self.get_entry(&child_node)?;

            let delimiter = if prefix.is_empty() { "" } else { "/" };

            // construct full path as Tree key is only one chunk of it
            let fullpath = self.key_to_string(prefix) + delimiter + key;
            self.get_key_values_from_tree_recursively(&fullpath, &entry, &mut keyvalues)?;
        }

        if keyvalues.is_empty() {
            Ok(None)
        } else {
            Ok(Some(keyvalues))
        }
    }

    /// Flush the working tree and and move to work on a certain commit from history.
    pub fn checkout(&mut self, context_hash: &EntryHash) -> Result<(), MerkleError> {
        let stat_updater = StatUpdater::new(MerkleStorageAction::Checkout, None);
        let commit = self.get_commit(&context_hash)?;
        let entry = self.get_entry_from_hash(&commit.root_hash)?;
        let tree = self.get_tree(&entry)?.clone();

        self.trees = HashMap::new();
        self.set_working_tree_root(tree, 0);
        self.last_commit_hash = Some(context_hash.clone());

        stat_updater.update_execution_stats(&mut self.stats);
        Ok(())
    }

    /// Take the current changes in the working tree, create a commit and persist all changes
    /// to database under the new commit. Return last commit if there are no changes, that is
    /// empty commits are not allowed.
    pub fn commit(
        &mut self,
        time: u64,
        author: String,
        message: String,
    ) -> Result<EntryHash, MerkleError> {
        let stat_updater = StatUpdater::new(MerkleStorageAction::Commit, None);
        let root_hash = self.get_working_tree_root_hash()?;
        let parent_commit_hash = self.last_commit_hash;

        let new_commit = Commit {
            root_hash,
            parent_commit_hash,
            time,
            author,
            message,
        };
        let new_commit_hash = hash_commit(&new_commit)?;
        let entry = Entry::Commit(new_commit);

        // persist working tree entries to db
        let mut batch: Vec<(EntryHash, ContextValue)> = Vec::new();
        self.get_entries_recursively(&entry, Some(&self.working_tree.0), &mut batch)?;
        // write all entries at once (depends on backend)
        self.db.write_batch(batch)?;

        self.last_commit_hash = Some(new_commit_hash);
        let rv = Ok(new_commit_hash);

        stat_updater.update_execution_stats(&mut self.stats);
        rv
    }

    /// Set key/val to the working tree.
    fn set_working_tree_root(&mut self, tree: Tree, tree_id: TreeId) {
        self.working_tree = (tree.clone(), tree_id);
        self.trees.insert(tree_id, tree);
    }

    /// Set key/val to the working tree.
    pub fn set(
        &mut self,
        new_tree_id: TreeId,
        key: &ContextKey,
        value: ContextValue,
    ) -> Result<(), MerkleError> {
        let stat_updater = StatUpdater::new(MerkleStorageAction::Set, Some(key));

        let entry = &self._set(key, value)?;
        let new_root = self.get_tree(entry)?.clone();
        self.set_working_tree_root(new_root, new_tree_id);

        stat_updater.update_execution_stats(&mut self.stats);
        Ok(())
    }

    fn _set(&mut self, key: &ContextKey, value: ContextValue) -> Result<Entry, MerkleError> {
        let entry = RefCell::new(Some(Entry::Blob(value)));
        let new_node = Node {
            entry,
            entry_hash: RefCell::new(None),
            node_kind: NodeKind::Leaf,
        };
        self.compute_new_root_with_change(&key, Some(new_node))
    }

    /// Delete an item from the working tree.
    pub fn delete(&mut self, new_tree_id: TreeId, key: &ContextKey) -> Result<(), MerkleError> {
        let stat_updater = StatUpdater::new(MerkleStorageAction::Delete, Some(key));

        let entry = &self._delete(key)?;
        let new_root = self.get_tree(entry)?.clone();
        self.set_working_tree_root(new_root, new_tree_id);

        stat_updater.update_execution_stats(&mut self.stats);
        Ok(())
    }

    fn _delete(&mut self, key: &ContextKey) -> Result<Entry, MerkleError> {
        if key.is_empty() {
            let root = self.get_working_tree_root();
            return Ok(Entry::Tree(root));
        }
        self.compute_new_root_with_change(&key, None)
    }

    /// Copy subtree under a new path.
    pub fn copy(
        &mut self,
        new_tree_id: TreeId,
        from_key: &ContextKey,
        to_key: &ContextKey,
    ) -> Result<(), MerkleError> {
        let stat_updater = StatUpdater::new(MerkleStorageAction::Copy, Some(from_key));

        let entry = self._copy(from_key, to_key)?;
        let new_root = self.get_tree(&entry)?.clone();
        self.set_working_tree_root(new_root, new_tree_id);

        stat_updater.update_execution_stats(&mut self.stats);
        Ok(())
    }

    fn _copy(&mut self, from_key: &ContextKey, to_key: &ContextKey) -> Result<Entry, MerkleError> {
        let root = self.get_working_tree_root_ref();
        let source_tree = self.find_tree(root, &from_key)?;

        Ok(self.compute_new_root_with_change(
            &to_key,
            Some(self.get_non_leaf(Entry::Tree(source_tree))),
        )?)
    }

    /// Get a new tree with `new_node` put under given `key`.
    /// Walk down the tree to find key, set new value and walk back up recalculating hashes -
    /// return new top hash of tree. Note: no writes to DB yet
    ///
    /// # Arguments
    ///
    /// * `root` - Tree to modify
    /// * `key` - path under which the changes takes place
    /// * `new_node` - None for deletion, Some for inserting a hash under the key.
    fn compute_new_root_with_change(
        &mut self,
        key: &[String],
        new_node: Option<Node>,
    ) -> Result<Entry, MerkleError> {
        let last = match key.last() {
            Some(last) => last,
            None => match new_node {
                Some(n) => {
                    // if there is a value we want to assigin - just
                    // assigin it
                    return n
                        .entry
                        .try_borrow()
                        .map_err(|_| MerkleError::InvalidState("Missing entry value"))?
                        .clone()
                        .ok_or(MerkleError::InvalidState("Missing entry value"));
                }
                None => {
                    // if key is empty and there is new_node == None
                    // that means that we just removed whole tree
                    // so set merkle storage root to empty dir and place
                    // it in working tree
                    return Ok(Entry::Tree(Tree::new()));
                }
            },
        };

        let path = &key[..key.len() - 1];
        let root = self.get_working_tree_root_ref();
        let mut tree = self.find_tree(root, path)?;

        match new_node {
            None => tree.remove(last),
            Some(new_node) => tree.insert(Arc::new(last.clone()), Arc::new(new_node)),
        };

        if tree.is_empty() {
            self.compute_new_root_with_change(path, None)
        } else {
            self.compute_new_root_with_change(path, Some(self.get_non_leaf(Entry::Tree(tree))))
        }
    }

    /// Find tree by path and return a copy. Return an empty tree if no tree under this path exists or if a blob
    /// (= value) is encountered along the way.
    ///
    /// # Arguments
    ///
    /// * `root` - reference to a tree in which we search
    /// * `key` - sought path
    fn find_tree(&self, root: &Tree, key: &[String]) -> Result<Tree, MerkleError> {
        let first = match key.first() {
            Some(first) => first,
            None => {
                // terminate recursion if end of path was reached
                return Ok(root.clone());
            }
        };

        // first get node at key
        let child_node = match root.get(first) {
            Some(hash) => hash,
            None => {
                return Ok(Tree::new());
            }
        };

        // get entry by hash (from working tree or DB)
        match self.get_entry(&child_node)? {
            Entry::Tree(tree) => self.find_tree(&tree, &key[1..]),
            Entry::Blob(_) => Ok(Tree::new()),
            Entry::Commit { .. } => Err(MerkleError::FoundUnexpectedStructure {
                sought: "Tree/Blob".to_string(),
                found: "commit".to_string(),
            }),
        }
    }

    /// Get current working tree. If it's empty, init genesis  and return genesis root.
    fn get_working_tree_root(&self) -> Tree {
        self.working_tree.0.clone()
    }

    fn get_working_tree_root_ref(&self) -> &Tree {
        &self.working_tree.0
    }

    pub fn get_working_tree_root_hash(&self) -> Result<EntryHash, MerkleError> {
        // TOOD: unnecessery recalculation, should be one when set_working_tree_root
        hash_tree(&self.working_tree.0).map_err(MerkleError::from)
    }

    pub fn working_tree_checkout(&mut self, tree_id: TreeId) -> Result<(), MerkleError> {
        if tree_id == self.working_tree.1 {
            return Ok(());
        }

        let tree = self
            .trees
            .get(&tree_id)
            .ok_or(MerkleError::TreeNotFoundInWorkingTree { tree_id })?
            .clone();

        self.set_working_tree_root(tree, tree_id);
        Ok(())
    }

    /// Marks all the entries from last commit as used
    /// so GC can know when to remove them
    pub fn block_applied(&mut self) -> Result<(), MerkleError> {
        let stat_updater = StatUpdater::new(MerkleStorageAction::BlockApplied, None);
        let rv = match self.last_commit_hash {
            Some(hash) => Ok(self.db.block_applied(hash)?),
            None => Err(MerkleError::GCTriggeredBeforeFirstCommit),
        };
        stat_updater.update_execution_stats(&mut self.stats);
        rv
    }

    /// Notify GC about new cycle
    pub fn start_new_cycle(&mut self) -> Result<(), MerkleError> {
        Ok(self.db.new_cycle_started()?)
    }

    /// Builds vector of entries to be persisted to DB, recursively
    fn get_entries_recursively(
        &self,
        entry: &Entry,
        root: Option<&Tree>,
        batch: &mut Vec<(EntryHash, ContextValue)>,
    ) -> Result<(), MerkleError> {
        // add entry to batch
        let hashed = hash_entry(entry)?;
        let serialized = bincode::serialize(entry)?;

        batch.push((hashed, serialized));

        match entry {
            Entry::Blob(_) => Ok(()),
            Entry::Tree(tree) => {
                // Go through all descendants and gather errors. Remap error if there is a failure
                // anywhere in the recursion paths. TODO: is revert possible?
                tree.iter()
                    .map(|(_, child_node)| {
                        match child_node
                            .entry
                            .try_borrow()
                            .map_err(|_| MerkleError::InvalidState("Entry borrows twice"))?
                            .as_ref()
                        {
                            None => Ok(()),
                            Some(entry) => self.get_entries_recursively(entry, None, batch),
                        }
                    })
                    .find_map(|res| match res {
                        Ok(_) => None,
                        Err(err) => Some(Err(err)),
                    })
                    .unwrap_or(Ok(()))
            }
            Entry::Commit(commit) => {
                let entry = match root {
                    Some(root) => Entry::Tree(root.clone()),
                    None => self.get_entry_from_hash(&commit.root_hash)?,
                };
                self.get_entries_recursively(&entry, None, batch)
            }
        }
    }

    fn get_tree<'e>(&self, entry: &'e Entry) -> Result<&'e Tree, MerkleError> {
        match entry {
            Entry::Tree(tree) => Ok(tree),
            Entry::Blob(_) => Err(MerkleError::FoundUnexpectedStructure {
                sought: "tree".to_string(),
                found: "blob".to_string(),
            }),
            Entry::Commit { .. } => Err(MerkleError::FoundUnexpectedStructure {
                sought: "tree".to_string(),
                found: "commit".to_string(),
            }),
        }
    }

    fn get_commit(&self, hash: &EntryHash) -> Result<Commit, MerkleError> {
        match self.get_entry_from_hash(hash)? {
            Entry::Commit(commit) => Ok(commit),
            Entry::Tree(_) => Err(MerkleError::FoundUnexpectedStructure {
                sought: "commit".to_string(),
                found: "tree".to_string(),
            }),
            Entry::Blob(_) => Err(MerkleError::FoundUnexpectedStructure {
                sought: "commit".to_string(),
                found: "blob".to_string(),
            }),
        }
    }

    fn get_entry(&self, node: &Node) -> Result<Entry, MerkleError> {
        if let Some(e) = node
            .entry
            .try_borrow()
            .map_err(|_| MerkleError::InvalidState("The Entry is borrowed more than once"))?
            .as_ref()
            .cloned()
        {
            return Ok(e);
        };

        let hash = node
            .entry_hash
            .try_borrow()
            .map_err(|_| MerkleError::InvalidState("The Entry hash is borrowed more than once"))?
            .as_ref()
            .copied()
            .ok_or(MerkleError::InvalidState("Missing entry hash"))?;

        let entry = self.get_entry_from_hash(&hash)?;

        node.entry
            .try_borrow_mut()
            .map_err(|_| MerkleError::InvalidState("The Entry is borrowed more than once"))?
            .replace(entry.clone());

        Ok(entry)
    }

    fn get_entry_from_hash(&self, hash: &EntryHash) -> Result<Entry, MerkleError> {
        let entry_bytes = self.db.get(&hash)?;
        match entry_bytes {
            None => Err(MerkleError::EntryNotFound {
                hash: HashType::ContextHash.hash_to_b58check(hash)?,
            }),
            Some(entry_bytes) => Ok(bincode::deserialize(&entry_bytes)?),
        }
    }

    fn get_non_leaf(&self, entry: Entry) -> Node {
        Node {
            node_kind: NodeKind::NonLeaf,
            entry_hash: RefCell::new(None),
            entry: RefCell::new(Some(entry)),
        }
    }

    /// Convert key in array form to string form
    fn key_to_string(&self, key: &ContextKey) -> String {
        key.join("/")
    }

    /// Convert key in string form to array form
    fn string_to_key(&self, string: &str) -> ContextKey {
        string.split('/').map(str::to_string).collect()
    }

    /// Get last committed hash
    pub fn get_last_commit_hash(&self) -> Option<EntryHash> {
        self.last_commit_hash
    }

    /// Get various merkle storage statistics
    pub fn get_merkle_stats(&self) -> Result<MerkleStoragePerfReport, MerkleError> {
        Ok(MerkleStoragePerfReport {
            perf_stats: self.stats.perf_stats.clone(),
            kv_store_stats: self.db.total_get_mem_usage()?,
        })
    }

    pub fn get_memory_usage(&self) -> Result<usize, MerkleError> {
        Ok(self.db.total_get_mem_usage()?)
    }

    pub fn get_block_latency(&self, offset_from_last_applied: usize) -> Option<u64> {
        self.stats.block_latencies.get(offset_from_last_applied)
    }

    fn flush_db(&self) -> Result<(), Error> {
        self.db.flush()
    }
}

impl Flushable for MerkleStorage {
    fn flush(&self) -> Result<(), Error> {
        self.flush_db()
    }
}

/// Merkle storage predefined tests with abstraction for underlaying kv_store for context
#[cfg(test)]
mod tests {
    use std::env;
    use std::path::PathBuf;

    use assert_json_diff::assert_json_eq;

    use crate::context::kv_store::test_support::TestContextKvStoreFactoryInstance;
    use crate::context::kv_store::SupportedContextKeyValueStore;
    use crate::context::ContextValue;

    use super::*;

    fn test_duplicate_entry_in_working_tree(kv_store_factory: &TestContextKvStoreFactoryInstance) {
        let mut storage = MerkleStorage::new(
            kv_store_factory
                .create("test_duplicate_entry_in_working_tree")
                .unwrap(),
        );

        let a_foo: &ContextKey = &vec!["a".to_string(), "foo".to_string()];
        let c_foo: &ContextKey = &vec!["c".to_string(), "foo".to_string()];
        storage
            .set(1, &vec!["a".to_string(), "foo".to_string()], vec![97, 98])
            .unwrap();
        storage
            .set(2, &vec!["c".to_string(), "zoo".to_string()], vec![1, 2])
            .unwrap();
        storage
            .set(3, &vec!["c".to_string(), "foo".to_string()], vec![97, 98])
            .unwrap();
        storage
            .delete(4, &vec!["c".to_string(), "zoo".to_string()])
            .unwrap();
        // now c/ is the same tree as a/ - which means there are two references to single entry in working tree
        // modify the tree and check that the other one was kept intact
        storage
            .set(5, &vec!["c".to_string(), "foo".to_string()], vec![3, 4])
            .unwrap();
        let commit = storage
            .commit(0, "Tezos".to_string(), "Genesis".to_string())
            .unwrap();
        assert_eq!(storage.get_history(&commit, a_foo).unwrap(), vec![97, 98]);
        assert_eq!(storage.get_history(&commit, c_foo).unwrap(), vec![3, 4]);
    }

    fn test_tree_hash(kv_store_factory: &TestContextKvStoreFactoryInstance) {
        let mut storage = MerkleStorage::new(kv_store_factory.create("test_tree_hash").unwrap());

        storage
            .set(
                1,
                &vec!["a".to_string(), "foo".to_string()],
                vec![97, 98, 99],
            )
            .unwrap(); // abc
        storage
            .set(2, &vec!["b".to_string(), "boo".to_string()], vec![97, 98])
            .unwrap();
        storage
            .set(
                3,
                &vec!["a".to_string(), "aaa".to_string()],
                vec![97, 98, 99, 100],
            )
            .unwrap();
        storage.set(4, &vec!["x".to_string()], vec![97]).unwrap();
        storage
            .set(
                5,
                &vec!["one".to_string(), "two".to_string(), "three".to_string()],
                vec![97],
            )
            .unwrap();
        storage
            .commit(0, "Tezos".to_string(), "Genesis".to_string())
            .unwrap();

        let hash = storage.get_working_tree_root_hash().unwrap();

        assert_eq!([0xDB, 0xAE, 0xD7, 0xB6], hash[0..4]);
    }

    fn test_commit_hash(kv_store_factory: &TestContextKvStoreFactoryInstance) {
        let mut storage = MerkleStorage::new(kv_store_factory.create("test_commit_hash").unwrap());

        storage
            .set(1, &vec!["a".to_string()], vec![97, 98, 99])
            .unwrap();

        let commit = storage.commit(0, "Tezos".to_string(), "Genesis".to_string());

        assert_eq!([0xCF, 0x95, 0x18, 0x33], commit.unwrap()[0..4]);

        storage
            .set(1, &vec!["data".to_string(), "x".to_string()], vec![97])
            .unwrap();
        let commit = storage.commit(0, "Tezos".to_string(), "".to_string());

        assert_eq!([0xCA, 0x7B, 0xC7, 0x02], commit.unwrap()[0..4]);
        // full irmin hash: ca7bc7022ffbd35acc97f7defb00c486bb7f4d19a2d62790d5949775eb74f3c8
    }

    fn test_multiple_commit_hash(kv_store_factory: &TestContextKvStoreFactoryInstance) {
        let mut storage = MerkleStorage::new(
            kv_store_factory
                .create("test_multiple_commit_hash")
                .unwrap(),
        );

        let _commit = storage.commit(0, "Tezos".to_string(), "Genesis".to_string());

        storage
            .set(
                1,
                &vec!["data".to_string(), "a".to_string(), "x".to_string()],
                vec![97],
            )
            .unwrap();
        storage
            .copy(
                2,
                &vec!["data".to_string(), "a".to_string()],
                &vec!["data".to_string(), "b".to_string()],
            )
            .unwrap();
        storage
            .delete(
                3,
                &vec!["data".to_string(), "b".to_string(), "x".to_string()],
            )
            .unwrap();
        let commit = storage.commit(0, "Tezos".to_string(), "".to_string());

        assert_eq!([0x9B, 0xB0, 0x0D, 0x6E], commit.unwrap()[0..4]);
    }

    fn test_get(kv_store_factory: &TestContextKvStoreFactoryInstance) {
        let db_name = "test_get";

        let key_abc: &ContextKey = &vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let key_abx: &ContextKey = &vec!["a".to_string(), "b".to_string(), "x".to_string()];
        let key_eab: &ContextKey = &vec!["e".to_string(), "a".to_string(), "b".to_string()];
        let key_az: &ContextKey = &vec!["a".to_string(), "z".to_string()];
        let key_d: &ContextKey = &vec!["d".to_string()];

        let kv_store = kv_store_factory.create(db_name).unwrap();
        let mut storage = MerkleStorage::new(kv_store);

        let res = storage.get(&vec![]);
        assert_eq!(res.unwrap().is_empty(), true);
        let res = storage.get(&vec!["a".to_string()]);
        assert_eq!(res.unwrap().is_empty(), true);

        storage.set(1, key_abc, vec![1u8, 2u8]).unwrap();
        storage.set(2, key_abx, vec![3u8]).unwrap();
        assert_eq!(storage.get(&key_abc).unwrap(), vec![1u8, 2u8]);
        assert_eq!(storage.get(&key_abx).unwrap(), vec![3u8]);
        let commit1 = storage.commit(0, "".to_string(), "".to_string()).unwrap();

        storage.set(3, key_az, vec![4u8]).unwrap();
        storage.set(4, key_abx, vec![5u8]).unwrap();
        storage.set(5, key_d, vec![6u8]).unwrap();
        storage.set(6, key_eab, vec![7u8]).unwrap();
        assert_eq!(storage.get(key_az).unwrap(), vec![4u8]);
        assert_eq!(storage.get(key_abx).unwrap(), vec![5u8]);
        assert_eq!(storage.get(key_d).unwrap(), vec![6u8]);
        assert_eq!(storage.get(key_eab).unwrap(), vec![7u8]);
        let commit2 = storage.commit(0, "".to_string(), "".to_string()).unwrap();

        assert_eq!(
            storage.get_history(&commit1, key_abc).unwrap(),
            vec![1u8, 2u8]
        );
        assert_eq!(storage.get_history(&commit1, key_abx).unwrap(), vec![3u8]);
        assert_eq!(storage.get_history(&commit2, key_abx).unwrap(), vec![5u8]);
        assert_eq!(storage.get_history(&commit2, key_az).unwrap(), vec![4u8]);
        assert_eq!(storage.get_history(&commit2, key_d).unwrap(), vec![6u8]);
        assert_eq!(storage.get_history(&commit2, key_eab).unwrap(), vec![7u8]);
    }

    fn test_mem(kv_store_factory: &TestContextKvStoreFactoryInstance) {
        let mut storage = MerkleStorage::new(kv_store_factory.create("test_mem").unwrap());

        let key_abc: &ContextKey = &vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let key_abx: &ContextKey = &vec!["a".to_string(), "b".to_string(), "x".to_string()];

        assert_eq!(storage.mem(&key_abc).unwrap(), false);
        assert_eq!(storage.mem(&key_abx).unwrap(), false);
        storage.set(1, key_abc, vec![1u8, 2u8]).unwrap();
        assert_eq!(storage.mem(&key_abc).unwrap(), true);
        assert_eq!(storage.mem(&key_abx).unwrap(), false);
        storage.set(2, key_abx, vec![3u8]).unwrap();
        assert_eq!(storage.mem(&key_abc).unwrap(), true);
        assert_eq!(storage.mem(&key_abx).unwrap(), true);
        storage.delete(3, key_abx).unwrap();
        assert_eq!(storage.mem(&key_abc).unwrap(), true);
        assert_eq!(storage.mem(&key_abx).unwrap(), false);
    }

    fn test_dirmem(kv_store_factory: &TestContextKvStoreFactoryInstance) {
        let mut storage = MerkleStorage::new(kv_store_factory.create("test_dirmem").unwrap());

        let key_abc: &ContextKey = &vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let key_ab: &ContextKey = &vec!["a".to_string(), "b".to_string()];
        let key_a: &ContextKey = &vec!["a".to_string()];

        assert_eq!(storage.dirmem(&key_a).unwrap(), false);
        assert_eq!(storage.dirmem(&key_ab).unwrap(), false);
        assert_eq!(storage.dirmem(&key_abc).unwrap(), false);
        storage.set(1, key_abc, vec![1u8, 2u8]).unwrap();
        assert_eq!(storage.dirmem(&key_a).unwrap(), true);
        assert_eq!(storage.dirmem(&key_ab).unwrap(), true);
        assert_eq!(storage.dirmem(&key_abc).unwrap(), false);
        storage.delete(2, key_abc).unwrap();
        assert_eq!(storage.dirmem(&key_a).unwrap(), false);
        assert_eq!(storage.dirmem(&key_ab).unwrap(), false);
        assert_eq!(storage.dirmem(&key_abc).unwrap(), false);
    }

    fn test_copy(kv_store_factory: &TestContextKvStoreFactoryInstance) {
        let mut storage = MerkleStorage::new(kv_store_factory.create("test_copy").unwrap());

        let key_abc: &ContextKey = &vec!["a".to_string(), "b".to_string(), "c".to_string()];
        storage.set(1, key_abc, vec![1_u8]).unwrap();
        storage
            .copy(2, &vec!["a".to_string()], &vec!["z".to_string()])
            .unwrap();

        assert_eq!(
            vec![1_u8],
            storage
                .get(&vec!["z".to_string(), "b".to_string(), "c".to_string()])
                .unwrap()
        );
        // TODO test copy over commits
    }

    fn test_delete(kv_store_factory: &TestContextKvStoreFactoryInstance) {
        let mut storage = MerkleStorage::new(kv_store_factory.create("test_delete").unwrap());

        let key_abc: &ContextKey = &vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let key_abx: &ContextKey = &vec!["a".to_string(), "b".to_string(), "x".to_string()];
        storage.set(1, key_abc, vec![2_u8]).unwrap();
        storage.set(2, key_abx, vec![3_u8]).unwrap();
        storage.delete(3, key_abx).unwrap();
        let commit1 = storage.commit(0, "".to_string(), "".to_string()).unwrap();

        assert!(storage.get_history(&commit1, &key_abx).is_err());
    }

    fn test_deleted_entry_available(kv_store_factory: &TestContextKvStoreFactoryInstance) {
        let mut storage = MerkleStorage::new(
            kv_store_factory
                .create("test_deleted_entry_available")
                .unwrap(),
        );

        let key_abc: &ContextKey = &vec!["a".to_string(), "b".to_string(), "c".to_string()];
        storage.set(1, key_abc, vec![2_u8]).unwrap();
        let commit1 = storage.commit(0, "".to_string(), "".to_string()).unwrap();
        storage.delete(2, key_abc).unwrap();
        let _commit2 = storage.commit(0, "".to_string(), "".to_string()).unwrap();

        assert_eq!(vec![2_u8], storage.get_history(&commit1, &key_abc).unwrap());
    }

    fn test_delete_in_separate_commit(kv_store_factory: &TestContextKvStoreFactoryInstance) {
        let mut storage = MerkleStorage::new(
            kv_store_factory
                .create("test_delete_in_separate_commit")
                .unwrap(),
        );

        let key_abc: &ContextKey = &vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let key_abx: &ContextKey = &vec!["a".to_string(), "b".to_string(), "x".to_string()];
        storage.set(1, key_abc, vec![2_u8]).unwrap();
        storage.set(2, key_abx, vec![3_u8]).unwrap();
        storage.commit(0, "".to_string(), "".to_string()).unwrap();

        storage.delete(1, key_abx).unwrap();
        let commit2 = storage.commit(0, "".to_string(), "".to_string()).unwrap();

        assert!(storage.get_history(&commit2, &key_abx).is_err());
    }

    fn test_checkout(kv_store_factory: &TestContextKvStoreFactoryInstance) {
        let key_abc: &ContextKey = &vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let key_abx: &ContextKey = &vec!["a".to_string(), "b".to_string(), "x".to_string()];

        let mut storage = MerkleStorage::new(kv_store_factory.create("test_checkout").unwrap());

        storage.set(1, key_abc, vec![1u8]).unwrap();
        storage.set(2, key_abx, vec![2u8]).unwrap();
        let commit1 = storage.commit(0, "".to_string(), "".to_string()).unwrap();

        storage.set(1, key_abc, vec![3u8]).unwrap();
        storage.set(2, key_abx, vec![4u8]).unwrap();
        let commit2 = storage.commit(0, "".to_string(), "".to_string()).unwrap();

        storage.checkout(&commit1).unwrap();
        assert_eq!(storage.get(&key_abc).unwrap(), vec![1u8]);
        assert_eq!(storage.get(&key_abx).unwrap(), vec![2u8]);
        // this set be wiped by checkout
        storage.set(1, key_abc, vec![8u8]).unwrap();

        storage.checkout(&commit2).unwrap();
        assert_eq!(storage.get(&key_abc).unwrap(), vec![3u8]);
        assert_eq!(storage.get(&key_abx).unwrap(), vec![4u8]);
    }

    /// Test getting entire tree in string format for JSON RPC
    fn test_get_context_tree_by_prefix(kv_store_factory: &TestContextKvStoreFactoryInstance) {
        let mut storage = MerkleStorage::new(
            kv_store_factory
                .create("test_get_context_tree_by_prefix")
                .unwrap(),
        );

        let all_json = serde_json::json!(
            {
                "adata": {
                    "b": {
                            "x": {
                                    "y":"090a"
                            }
                    }
                },
                "data": {
                    "a": {
                            "x": {
                                    "y":"0506"
                            }
                    },
                    "b": {
                            "x": {
                                    "y":"0708"
                            }
                    },
                    "c":"0102"
                }
            }
        );
        let data_json = serde_json::json!(
            {
                "a": {
                        "x": {
                                "y":"0506"
                        }
                },
                "b": {
                        "x": {
                                "y":"0708"
                        }
                },
                "c":"0102"
            }
        );

        let _commit = storage
            .commit(0, "Tezos".to_string(), "Genesis".to_string())
            .unwrap();

        storage
            .set(
                1,
                &vec!["data".to_string(), "a".to_string(), "x".to_string()],
                vec![3, 4],
            )
            .unwrap();
        storage
            .set(2, &vec!["data".to_string(), "a".to_string()], vec![1, 2])
            .unwrap();
        storage
            .set(
                3,
                &vec![
                    "data".to_string(),
                    "a".to_string(),
                    "x".to_string(),
                    "y".to_string(),
                ],
                vec![5, 6],
            )
            .unwrap();
        storage
            .set(
                4,
                &vec![
                    "data".to_string(),
                    "b".to_string(),
                    "x".to_string(),
                    "y".to_string(),
                ],
                vec![7, 8],
            )
            .unwrap();
        storage
            .set(5, &vec!["data".to_string(), "c".to_string()], vec![1, 2])
            .unwrap();
        storage
            .set(
                6,
                &vec![
                    "adata".to_string(),
                    "b".to_string(),
                    "x".to_string(),
                    "y".to_string(),
                ],
                vec![9, 10],
            )
            .unwrap();
        //data-a[1,2]
        //data-a-x[3,4]
        //data-a-x-y[5,6]
        //data-b-x-y[7,8]
        //data-c[1,2]
        //adata-b-x-y[9,10]
        let commit = storage.commit(0, "Tezos".to_string(), "Genesis".to_string());

        // without depth
        let rv_all = storage
            .get_context_tree_by_prefix(&commit.as_ref().unwrap(), &vec![], None)
            .unwrap();
        assert_json_eq!(all_json, serde_json::to_value(&rv_all).unwrap());

        let rv_data = storage
            .get_context_tree_by_prefix(&commit.as_ref().unwrap(), &vec!["data".to_string()], None)
            .unwrap();
        assert_json_eq!(data_json, serde_json::to_value(&rv_data).unwrap());

        // with depth 0
        assert_json_eq!(
            serde_json::json!(null),
            serde_json::to_value(
                storage
                    .get_context_tree_by_prefix(&commit.as_ref().unwrap(), &vec![], Some(0))
                    .unwrap()
            )
            .unwrap()
        );

        // with depth 1
        assert_json_eq!(
            serde_json::json!(
                {
                    "adata": null,
                    "data": null
                }
            ),
            serde_json::to_value(
                storage
                    .get_context_tree_by_prefix(&commit.as_ref().unwrap(), &vec![], Some(1))
                    .unwrap()
            )
            .unwrap()
        );
        // with depth 2
        assert_json_eq!(
            serde_json::json!(
                {
                    "adata": {
                        "b" : null
                    },
                    "data": {
                        "a" : null,
                        "b" : null,
                        "c" : null,
                    },
                }
            ),
            serde_json::to_value(
                storage
                    .get_context_tree_by_prefix(&commit.as_ref().unwrap(), &vec![], Some(2))
                    .unwrap()
            )
            .unwrap()
        );
    }

    fn test_backtracking_on_set(kv_store_factory: &TestContextKvStoreFactoryInstance) {
        let mut storage =
            MerkleStorage::new(kv_store_factory.create("test_backtracking_on_set").unwrap());

        let dummy_key = &vec!["a".to_string()];
        storage.set(1, dummy_key, vec![1u8]).unwrap();
        storage.set(2, dummy_key, vec![2u8]).unwrap();

        // get recent value
        assert_eq!(storage.get(dummy_key).unwrap(), vec![2u8]);

        // checkout previous stage state
        storage.working_tree_checkout(1).unwrap();
        assert_eq!(storage.get(dummy_key).unwrap(), vec![1u8]);

        // checkout newest stage state
        storage.working_tree_checkout(2).unwrap();
        assert_eq!(storage.get(dummy_key).unwrap(), vec![2u8]);
    }

    fn test_backtracking_on_delete(kv_store_factory: &TestContextKvStoreFactoryInstance) {
        let mut storage = MerkleStorage::new(
            kv_store_factory
                .create("test_backtracking_on_delete")
                .unwrap(),
        );

        let key = &vec!["a".to_string()];
        let value = vec![1u8];
        let empty_response: ContextValue = Vec::new();

        storage.set(1, key, value.clone()).unwrap();
        storage.delete(2, key).unwrap();

        assert_eq!(storage.get(key).unwrap(), empty_response);

        // // checkout previous stage state
        storage.working_tree_checkout(1).unwrap();
        assert_eq!(storage.get(key).unwrap(), value);

        // checkout latest stage state
        storage.working_tree_checkout(2).unwrap();
        assert_eq!(storage.get(key).unwrap(), empty_response);
    }

    // Currently we don't perform a cleanup after each COMMIT
    // That will happen during the next CHECKOUT, this test is to ensure that
    fn test_checkout_stage_from_before_commit(
        kv_store_factory: &TestContextKvStoreFactoryInstance,
    ) {
        let mut storage = MerkleStorage::new(
            kv_store_factory
                .create("test_checkout_stage_from_before_commit")
                .unwrap(),
        );

        let key = &vec!["a".to_string()];
        storage.set(1, key, vec![1u8]).unwrap();
        storage.set(2, key, vec![2u8]).unwrap();
        storage
            .commit(0, "author".to_string(), "message".to_string())
            .unwrap();

        assert_eq!(storage.working_tree_checkout(1).is_err(), false);
    }

    macro_rules! tests_with_storage {
        ($storage_tests_name:ident, $kv_store_factory:expr) => {
            mod $storage_tests_name {
                #[test]
                fn test_tree_hash() {
                    super::test_tree_hash($kv_store_factory)
                }
                #[test]
                fn test_duplicate_entry_in_working_tree() {
                    super::test_duplicate_entry_in_working_tree($kv_store_factory)
                }
                #[test]
                fn test_commit_hash() {
                    super::test_commit_hash($kv_store_factory)
                }
                #[test]
                fn test_multiple_commit_hash() {
                    super::test_multiple_commit_hash($kv_store_factory)
                }
                #[test]
                fn test_get() {
                    super::test_get($kv_store_factory)
                }
                #[test]
                fn test_mem() {
                    super::test_mem($kv_store_factory)
                }
                #[test]
                fn test_dirmem() {
                    super::test_dirmem($kv_store_factory)
                }
                #[test]
                fn test_copy() {
                    super::test_copy($kv_store_factory)
                }
                #[test]
                fn test_delete() {
                    super::test_delete($kv_store_factory)
                }
                #[test]
                fn test_deleted_entry_available() {
                    super::test_deleted_entry_available($kv_store_factory)
                }
                #[test]
                fn test_delete_in_separate_commit() {
                    super::test_delete_in_separate_commit($kv_store_factory)
                }
                #[test]
                fn test_checkout() {
                    super::test_checkout($kv_store_factory)
                }
                #[test]
                fn test_get_context_tree_by_prefix() {
                    super::test_get_context_tree_by_prefix($kv_store_factory)
                }
                #[test]
                fn test_backtracking_on_set() {
                    super::test_backtracking_on_set($kv_store_factory)
                }
                #[test]
                fn test_backtracking_on_delete() {
                    super::test_backtracking_on_delete($kv_store_factory)
                }
                #[test]
                fn test_fail_to_checkout_stage_from_before_commit() {
                    super::test_checkout_stage_from_before_commit($kv_store_factory)
                }
            }
        };
    }

    lazy_static::lazy_static! {
        static ref SUPPORTED_KV_STORES: std::collections::HashMap<SupportedContextKeyValueStore, TestContextKvStoreFactoryInstance> = crate::context::kv_store::test_support::all_kv_stores(out_dir_path());
    }

    fn out_dir_path() -> PathBuf {
        let out_dir = env::var("OUT_DIR").expect(
            "OUT_DIR is not defined - please add build.rs to root or set env variable OUT_DIR",
        );
        out_dir.as_str().into()
    }

    macro_rules! tests_with_all_kv_stores {
        () => {
            tests_with_storage!(
                kv_store_inmemory_tests,
                super::SUPPORTED_KV_STORES
                    .get(&crate::context::kv_store::SupportedContextKeyValueStore::InMem)
                    .unwrap()
            );
            tests_with_storage!(
                kv_store_btree_tests,
                super::SUPPORTED_KV_STORES
                    .get(&crate::context::kv_store::SupportedContextKeyValueStore::BTreeMap)
                    .unwrap()
            );
            tests_with_storage!(
                kv_store_rocksdb_tests,
                super::SUPPORTED_KV_STORES
                    .get(
                        &crate::context::kv_store::SupportedContextKeyValueStore::RocksDB {
                            path: super::out_dir_path()
                        }
                    )
                    .unwrap()
            );
            tests_with_storage!(
                kv_store_sled_tests,
                super::SUPPORTED_KV_STORES
                    .get(
                        &crate::context::kv_store::SupportedContextKeyValueStore::Sled {
                            path: super::out_dir_path()
                        }
                    )
                    .unwrap()
            );
        };
    }

    tests_with_all_kv_stores!();
}
