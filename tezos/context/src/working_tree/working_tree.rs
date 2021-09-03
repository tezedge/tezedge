// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

// TODO: these comments are not entirely up to date now (and haven't been for a while), update.

//! # WorkingTree
//!
//! Abstract merkle storage with git-like semantics and history which can be used with different K-V stores.
//!
//! # Data Structure
//! A storage with just one key `a/b/c` and its corresponding value `8` is represented like this:
//!
//! ``
//! [commit] ---> [dir1] --a--> [dir2] --b--> [dir3] --c--> [blob_8]
//! ``
//!
//! The db then contains the following:
//! ```no_compile
//! <hash_of_blob; blob8>
//! <hash_of_dir3, dir3>, where dir3 is a map {c: hash_blob8}
//! <hash_of_dir2, dir2>, where dir2 is a map {b: hash_of_dir3}
//! <hash_of_dir2, dir2>, where dir1 is a map {a: hash_of_dir2}
//! <hash_of_commit>; commit>, where commit points to the root directory (dir1)
//! ```
//!
//! Then, when looking for a path a/b/c in a spcific commit, we first get the hash of the root dir
//! from the commit, then get the dir from the database, get the hash of "a", look it up in the db,
//! get the hash of "b" from that dir, load from db, then get the hash of "c" and retrieve the
//! final value.
//!
//!
//! Now, let's assume we want to add a path `X` also referencing the value `8`. That creates a new
//! tree that reuses the previous subtree for `a/b/c` and branches away from root for `X`:
//!
//! ```no_compile
//! [tree1] --a--> [dir2] --b--> [dir3] --c--> [blob_8]
//!                   ^                             ^
//!                   |                             |
//! [tree_X]----a-----                              |
//!     |                                           |
//!      ----------------------X--------------------
//! ```
//!
//! The following is added to the database:
//! ``
//! <hash_of_dir_X; dir_X>, where dir_X is a map {a: hash_of_dir2, X: hash_of_blob8}
//! ``
//!
//! Reference: https://git-scm.com/book/en/v2/Git-Internals-Git-Objects
use std::{
    array::TryFromSliceError,
    sync::{Arc, PoisonError},
    vec::IntoIter,
};

use thiserror::Error;

use crypto::hash::FromBytesError;
use tezos_timing::SerializeStats;

use crate::{gc::GarbageCollectionError, tezedge_context::TezedgeIndex};
use crate::{hash::ObjectHash, ContextKeyOwned};
use crate::{
    hash::{hash_blob, hash_inlined_blob},
    working_tree::{Commit, DirEntry, DirEntryKind, Object},
};
use crate::{
    hash::{hash_commit, hash_directory, HashingError},
    kv_store::HashId,
};
use crate::{persistent, ContextKeyValueStore};
use crate::{ContextKey, ContextValue};

use super::{
    serializer::{deserialize, serialize_object, DeserializationError, SerializationError},
    storage::{BlobId, DirEntryId, DirectoryId, Storage, StorageError},
};

pub struct PostCommitData {
    pub commit_hash_id: HashId,
    pub batch: Vec<(HashId, Arc<[u8]>)>,
    pub reused: Vec<HashId>,
    pub serialize_stats: Box<SerializeStats>,
}

// The root of the 'working tree' can be either a Directory or a Value
#[derive(Clone)]
enum WorkingTreeRoot {
    Directory(DirectoryId),
    Value(BlobId),
}

#[derive(Clone)]
pub struct WorkingTree {
    root: WorkingTreeRoot,
    pub index: TezedgeIndex,
}

#[derive(Clone, Copy)]
pub enum FoldDepth {
    Eq(i64), // folds over nodes and contents of depth exactly [d].
    Lt(i64), // folds over nodes and contents of depth strictly less than [d].
    Le(i64), // folds over nodes and contents of depth less than or equal to [d].
    Gt(i64), // folds over nodes and contents of depth strictly more than [d].
    Ge(i64), // folds over nodes and contents of depth more than or equal to [d].
}

impl FoldDepth {
    /// `true` if for this depth the fold function should be applied
    fn should_apply(self, depth: i64) -> bool {
        match self {
            Self::Eq(n) => depth == n,
            Self::Lt(n) => depth < n,
            Self::Le(n) => depth <= n,
            Self::Gt(n) => depth > n,
            Self::Ge(n) => depth >= n,
        }
    }

    /// `true` if for this depth we should continue scanning the children
    fn should_continue(self, depth: i64) -> bool {
        match self {
            Self::Eq(n) => depth < n,
            Self::Lt(n) => depth < n - 1,
            Self::Le(n) => depth < n,
            Self::Gt(_) => true,
            Self::Ge(_) => true,
        }
    }
}

struct TreeWalkerLevel {
    key: ContextKeyOwned,
    root: WorkingTree,
    current_depth: i64,
    yield_self: bool,
    children_iter: Option<IntoIter<(String, DirEntryId)>>,
}

impl TreeWalkerLevel {
    fn new(
        key: ContextKeyOwned,
        root: WorkingTree,
        current_depth: i64,
        depth: &Option<FoldDepth>,
    ) -> Self {
        let should_continue = depth
            .map(|d| d.should_continue(current_depth))
            .unwrap_or(true);

        let children_iter = if should_continue {
            if let WorkingTreeRoot::Directory(dir_id) = &root.root {
                let storage = root.index.storage.borrow();

                let dir_len = match storage.dir_len(*dir_id) {
                    Ok(dir_len) => dir_len,
                    Err(e) => {
                        eprintln!("TreeWalkerLevel tree_len error='{:?}' key='{:?}", e, key);
                        0
                    }
                };

                let mut dir_vec = Vec::with_capacity(dir_len);

                storage
                    .dir_iterate_unsorted(*dir_id, |&(key_id, dir_entry_id)| {
                        let key = match storage.get_str(key_id) {
                            Ok(key) => key.to_string(),
                            Err(e) => {
                                // TODO: Handle this error in a better way
                                eprintln!("TreeWalkerLevel error='{:?}' key='{:?}", e, key);
                                return Ok(());
                            }
                        };
                        dir_vec.push((key, dir_entry_id));
                        Ok(())
                    })
                    .ok();

                Some(dir_vec.into_iter())
            } else {
                None
            }
        } else {
            None
        };

        Self {
            key,
            root,
            current_depth,
            yield_self: depth.map(|d| d.should_apply(current_depth)).unwrap_or(true),
            children_iter,
        }
    }
}

pub struct TreeWalker {
    depth: Option<FoldDepth>,
    stack: Vec<TreeWalkerLevel>,
}

impl TreeWalker {
    fn new(key: ContextKeyOwned, root: WorkingTree, depth: Option<FoldDepth>) -> Self {
        Self {
            depth,
            stack: vec![TreeWalkerLevel::new(key, root, 0, &depth)],
        }
    }

    fn empty() -> Self {
        Self {
            depth: None,
            stack: vec![],
        }
    }
}

impl Iterator for TreeWalker {
    type Item = (ContextKeyOwned, WorkingTree);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(current_level) = self.stack.last_mut() {
                if current_level.yield_self {
                    current_level.yield_self = false;
                    return Some((current_level.key.clone(), current_level.root.clone()));
                }

                if let Some(iter) = &mut current_level.children_iter {
                    let current_depth = current_level.current_depth + 1;

                    if let Some((k, dir_entry)) = iter.next() {
                        match current_level.root.dir_entry_tree(dir_entry) {
                            Ok(root) => {
                                // TODO: this is not very efficient, maybe we need to improve the key representation
                                let mut key = current_level.key.clone();
                                key.push(k.to_string());

                                self.stack.push(TreeWalkerLevel::new(
                                    key,
                                    root,
                                    current_depth,
                                    &self.depth,
                                ));
                            }
                            Err(e) => {
                                eprintln!(
                                    "TreeWalker Iterator error='{:?}' key='{:?}",
                                    e, current_level.key
                                );
                            }
                        }

                        continue;
                    }
                }

                self.stack.pop();
            } else {
                // Nothing left, we are done iterating
                return None;
            }
        }
    }
}

#[derive(Debug, Error)]
pub enum MerkleError {
    /// External libs errors
    #[error("RocksDB error: {error:?}")]
    DBError { error: persistent::DBError },
    #[error("Backend error: {error:?}")]
    GarbageCollectionError { error: GarbageCollectionError },

    /// Internal unrecoverable bugs that should never occur
    #[error("There is a commit or three under key {key:?}, but not a value!")]
    ValueIsNotABlob { key: String },
    #[error("Found wrong structure. Was looking for {sought}, but found {found}")]
    FoundUnexpectedStructure { sought: String, found: String },
    #[error("Object not found! HashId={hash_id:?}")]
    ObjectNotFound { hash_id: HashId },

    /// Wrong user input errors
    #[error("No value under key {key:?}.")]
    ValueNotFound { key: String },
    #[error("Cannot search for an empty key.")]
    KeyEmpty,
    #[error("Failed to convert hash into array: {error}")]
    HashToArrayError { error: TryFromSliceError },
    #[error("Failed to convert hash into string: {error}")]
    HashToStringError { error: FromBytesError },
    #[error("Failed to encode hash: {error}")]
    HashingError { error: HashingError },
    #[error("Expected value instead of `None` for {0}")]
    ValueExpected(&'static str),
    #[error("Invalid state: {0}")]
    InvalidState(&'static str),
    #[error("Mutex/lock error, reason: {reason:?}")]
    LockError { reason: String },
    #[error("Serialization error, {error:?}")]
    SerializationError { error: SerializationError },
    #[error("Deserialization error, {error:?}")]
    DeserializationError { error: DeserializationError },
    #[error("Storage ID error, {error:?}")]
    StorageIdError { error: StorageError },
}

impl From<persistent::DBError> for MerkleError {
    fn from(error: persistent::DBError) -> Self {
        MerkleError::DBError { error }
    }
}

impl From<HashingError> for MerkleError {
    fn from(error: HashingError) -> Self {
        Self::HashingError { error }
    }
}

impl From<SerializationError> for MerkleError {
    fn from(error: SerializationError) -> Self {
        Self::SerializationError { error }
    }
}

impl From<DeserializationError> for MerkleError {
    fn from(error: DeserializationError) -> Self {
        Self::DeserializationError { error }
    }
}

impl From<GarbageCollectionError> for MerkleError {
    fn from(error: GarbageCollectionError) -> Self {
        Self::GarbageCollectionError { error }
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

impl From<StorageError> for MerkleError {
    fn from(error: StorageError) -> Self {
        Self::StorageIdError { error }
    }
}

impl<T> From<PoisonError<T>> for MerkleError {
    fn from(pe: PoisonError<T>) -> Self {
        Self::LockError {
            reason: format!("{}", pe),
        }
    }
}

#[derive(Debug, Error)]
pub enum CheckObjectHashError {
    #[error("MerkleError error: {error:?}")]
    MerkleError { error: MerkleError },
    #[error("Calculated hash for {object_type} not matching expected hash: expected {expected:?}, calculated {calculated:?}")]
    InvalidHashError {
        object_type: String,
        calculated: String,
        expected: String,
    },
}

struct SerializingData<'a> {
    batch: Vec<(HashId, Arc<[u8]>)>,
    referenced_older_objects: Vec<HashId>,
    store: &'a mut ContextKeyValueStore,
    serialized: Vec<u8>,
    stats: Box<SerializeStats>,
}

impl<'a> SerializingData<'a> {
    fn new(store: &'a mut ContextKeyValueStore) -> Self {
        Self {
            batch: Vec::with_capacity(2048),
            referenced_older_objects: Vec::with_capacity(2048),
            store,
            serialized: Vec::with_capacity(2048),
            stats: Default::default(),
        }
    }

    fn add_serialized_object(
        &mut self,
        object_hash_id: HashId,
        object: &Object,
        storage: &Storage,
    ) -> Result<(), MerkleError> {
        serialize_object(
            object,
            object_hash_id,
            &mut self.serialized,
            storage,
            &mut self.stats,
            &mut self.batch,
            &mut self.referenced_older_objects,
        )?;
        Ok(())
    }

    fn add_older_object(
        &mut self,
        dir_entry: &DirEntry,
        storage: &Storage,
    ) -> Result<(), MerkleError> {
        let hash_id = dir_entry.object_hash_id(self.store, storage)?;

        if let Some(hash_id) = hash_id {
            self.referenced_older_objects.push(hash_id);
        };

        Ok(())
    }
}

impl WorkingTree {
    /// Creates a new working tree with an empty directory
    pub fn new(index: TezedgeIndex) -> Self {
        Self::new_with_directory(index, DirectoryId::empty())
    }

    /// Creates a new working tree with the directory `dir_id` as root.
    pub fn new_with_directory(index: TezedgeIndex, dir_id: DirectoryId) -> Self {
        WorkingTree {
            index,
            root: WorkingTreeRoot::Directory(dir_id),
        }
    }

    /// Creates a new working tree with the value `blob_id` as root.
    pub fn new_with_value(index: TezedgeIndex, blob_id: BlobId) -> Self {
        WorkingTree {
            index,
            root: WorkingTreeRoot::Value(blob_id),
        }
    }

    /// Returns the root value of this working tree.
    ///
    /// If the root is not a value (blob), this returns `None`
    pub fn get_value(&self) -> Option<ContextValue> {
        match self.root {
            WorkingTreeRoot::Directory(_) => None,
            WorkingTreeRoot::Value(blob_id) => {
                let storage = self.index.storage.borrow();
                storage.get_blob(blob_id).map(|v| v.to_vec()).ok()
            }
        }
    }

    /// Traverses this working tree and returns the tree at `key`.
    ///
    /// Fetches objects from repository if necessary.
    pub fn find_tree(&self, key: &ContextKey) -> Result<Option<Self>, MerkleError> {
        if key.is_empty() {
            // If the key is empty, we are checking self, which exists
            return Ok(Some(self.clone()));
        }

        let mut storage = self.index.storage.borrow_mut();
        let root = self.get_root_directory();

        let dir_entry_id = match self.index.find_dir_entry(root, key, &mut storage) {
            Ok(Some(dir_entry_id)) => dir_entry_id,
            _ => return Ok(None),
        };

        match self.index.dir_entry_object(dir_entry_id, &mut storage) {
            Err(MerkleError::ObjectNotFound { .. }) => Ok(None),
            Err(err) => Err(err),
            Ok(Object::Directory(dir_id)) => {
                Ok(Some(Self::new_with_directory(self.index.clone(), dir_id)))
            }
            Ok(Object::Blob(blob_id)) => {
                Ok(Some(Self::new_with_value(self.index.clone(), blob_id)))
            }
            Ok(Object::Commit(_)) => Err(MerkleError::FoundUnexpectedStructure {
                sought: "directory".to_string(),
                found: "commit".to_string(),
            }),
        }
    }

    /// Adds the `tree` at `key` in this working tree.
    pub fn add_tree(&self, key: &ContextKey, tree: &Self) -> Result<Self, MerkleError> {
        // If the tree is empty, we must instead remove that path
        if tree.is_empty() {
            self.delete(key)
        } else {
            let mut storage = self.index.storage.borrow_mut();

            let dir_entry = match tree.root {
                WorkingTreeRoot::Directory(dir_id) => {
                    DirEntry::new_directory(Object::Directory(dir_id))
                }
                WorkingTreeRoot::Value(blob_id) => DirEntry::new_blob(Object::Blob(blob_id)),
            };

            let object = &self._add(key, dir_entry, &mut storage)?;
            let dir_id = self.object_directory(object)?;

            Ok(self.with_new_root(dir_id))
        }
    }

    pub fn equal(&self, other: &Self) -> Result<bool, MerkleError> {
        // TODO: ok for now, but perform an actual compare instead of hashing here
        Ok(self.hash()? == other.hash()?)
    }

    /// Returns the root hash of this working tree.
    pub fn hash(&self) -> Result<ObjectHash, MerkleError> {
        let storage = self.index.storage.borrow();
        let mut repo = self.index.repository.write()?;

        let hash_id = match self.root {
            WorkingTreeRoot::Directory(_) => self.get_root_directory_hash(&mut *repo)?,
            WorkingTreeRoot::Value(blob_id) => match hash_blob(blob_id, &mut *repo, &storage)? {
                Some(hash_id) => hash_id,
                None => {
                    let blob = storage.get_blob(blob_id)?;
                    let hash = hash_inlined_blob(blob)?;
                    return Ok(hash);
                }
            },
        };

        match repo.get_hash(hash_id)? {
            Some(hash) => Ok(hash.into_owned()),
            None => Err(MerkleError::ObjectNotFound { hash_id }),
        }
    }

    /// Checks if the root of this working tree is a directory or a value.
    pub fn kind(&self) -> DirEntryKind {
        match &self.root {
            WorkingTreeRoot::Directory(_) => DirEntryKind::Directory,
            WorkingTreeRoot::Value(_) => DirEntryKind::Blob,
        }
    }

    /// Returns a new working tree, with an empty directory as root.
    pub fn empty(&self) -> Self {
        Self::new(self.index.clone())
    }

    /// Checks if the working tree is empty.
    ///
    /// If the root is a value, it is not empty.
    pub fn is_empty(&self) -> bool {
        match &self.root {
            WorkingTreeRoot::Directory(dir_id) => dir_id.is_empty(),
            WorkingTreeRoot::Value(_) => false,
        }
    }

    pub fn list(
        &self,
        offset: Option<usize>,
        length: Option<usize>,
        key: &ContextKey,
    ) -> Result<Vec<(String, WorkingTree)>, MerkleError> {
        let root = self.get_root_directory();
        let mut storage = self.index.storage.borrow_mut();
        let dir_id = self.find_or_create_directory(root, key, &mut storage)?;

        // It's important to get the directory sorted here
        let dir_entry = storage.dir_to_vec_sorted(dir_id)?;

        let dir_entry_length = dir_entry.len();
        let length = length.unwrap_or(dir_entry_length).min(dir_entry_length);
        let offset = offset.unwrap_or(0);

        let mut children = Vec::with_capacity(length);

        for (key, value) in dir_entry.iter().skip(offset).take(length) {
            let value = match self.dir_entry_object(*value, &mut storage)? {
                Object::Directory(dir_id) => Self::new_with_directory(self.index.clone(), dir_id),
                Object::Blob(blob_id) => Self::new_with_value(self.index.clone(), blob_id),
                Object::Commit(_) => continue,
            };

            let key = storage.get_str(*key)?.to_string();
            children.push((key, value));
        }

        Ok(children)
    }

    /// Returns `dir_entry_id` as root of a new working tree.
    ///
    /// Fetches the dir_entry from repository if necessary.
    fn dir_entry_tree(&self, dir_entry_id: DirEntryId) -> Result<Self, MerkleError> {
        let mut storage = self.index.storage.borrow_mut();

        let object = self.index.dir_entry_object(dir_entry_id, &mut storage)?;
        let dir_entry = storage.get_dir_entry(dir_entry_id)?;
        let tree = match dir_entry.dir_entry_kind() {
            DirEntryKind::Directory => WorkingTree {
                index: self.index.clone(),
                root: WorkingTreeRoot::Directory(self.object_directory(&object)?),
            },
            DirEntryKind::Blob => WorkingTree {
                index: self.index.clone(),
                root: WorkingTreeRoot::Value(self.object_value(&object)?),
            },
        };
        Ok(tree)
    }

    // From OCaml:
    /*
      [fold ?depth t root ~init ~f] recursively folds over the trees
      and values of [t]. The [f] callbacks are called with a key relative
      to [root]. [f] is never called with an empty key for values; i.e.,
      folding over a value is a no-op.

      Elements are traversed in lexical order of keys.

      The depth is 0-indexed. If [depth] is set (by default it is not), then [f]
      is only called when the conditions described by the parameter is true:

      - [Eq d] folds over nodes and contents of depth exactly [d].
      - [Lt d] folds over nodes and contents of depth strictly less than [d].
      - [Le d] folds over nodes and contents of depth less than or equal to [d].
      - [Gt d] folds over nodes and contents of depth strictly more than [d].
      - [Ge d] folds over nodes and contents of depth more than or equal to [d].
    */
    pub fn fold_iter(
        &self,
        depth: Option<FoldDepth>,
        key: &ContextKey,
    ) -> Result<TreeWalker, MerkleError> {
        if let Some(root) = self.find_tree(key)? {
            Ok(TreeWalker::new(
                vec![], // Key is relative to the root
                root,
                depth,
            ))
        } else {
            Ok(TreeWalker::empty())
        }
    }

    /// Get value at 'key' from this working tree.
    ///
    /// If object at `key` is missing or is a directory, this returns `None`.
    /// Fetches data from repository if necessary.
    pub fn find(&self, key: &ContextKey) -> Result<Option<ContextValue>, MerkleError> {
        let root = self.get_root_directory();
        let mut storage = self.index.storage.borrow_mut();

        match self.index.try_find_blob(root, key, &mut storage) {
            Ok(blob_id) => {
                let blob = storage.get_blob(blob_id)?;
                Ok(Some(blob.to_vec()))
            }
            Err(MerkleError::ValueNotFound { .. }) => Ok(None),
            Err(MerkleError::ValueIsNotABlob { .. }) => Ok(None),
            Err(err) => Err(err),
        }
    }

    /// Checks if a value (blob) at `key` exists in this working tree.
    ///
    /// Returns false if object at `key` is not a blob.
    pub fn mem(&self, key: &ContextKey) -> Result<bool, MerkleError> {
        let root = self.get_root_directory();
        self.value_exists(root, key)
    }

    /// Checks if a value (blob) or directory at `key` exists in this working tree.
    ///
    /// Returns false when path `key` doesn't exist.
    pub fn mem_tree(&self, key: &ContextKey) -> bool {
        let root = self.get_root_directory();
        self.dir_entry_exists(root, key)
    }

    /// Checks if a value (blob) at `key` exists in `dir_id`.
    ///
    /// Returns false if object at `key` is not a blob.
    fn value_exists(&self, dir_id: DirectoryId, key: &ContextKey) -> Result<bool, MerkleError> {
        if key.is_empty() {
            return Err(MerkleError::KeyEmpty);
        }

        let mut storage = self.index.storage.borrow_mut();

        let dir_entry_id = match self.index.find_dir_entry(dir_id, key, &mut storage) {
            Ok(Some(dir_entry_id)) => dir_entry_id,
            _ => return Ok(false),
        };

        let dir_entry = storage.get_dir_entry(dir_entry_id)?;
        Ok(dir_entry.dir_entry_kind() == DirEntryKind::Blob)
    }

    /// Checks if a value (blob) or directory at `key` exists in `dir_id`.
    ///
    /// Returns false when path `key` doesn't exist.
    fn dir_entry_exists(&self, dir_id: DirectoryId, key: &ContextKey) -> bool {
        if key.is_empty() {
            // If the key is empty, we are checking self, which exists
            return true;
        }

        let mut storage = self.index.storage.borrow_mut();

        match self.index.find_dir_entry(dir_id, key, &mut storage) {
            Ok(dir_entry_id) => dir_entry_id.is_some(),
            Err(_) => false,
        }
    }

    /// Take the current changes in the staging area, create a commit and persist all changes
    /// to database under the new commit. Return last commit if there are no changes, that is
    /// empty commits are not allowed.
    // FIXME: we don't check for empty commits.
    pub fn prepare_commit(
        &self,
        time: u64,
        author: String,
        message: String,
        parent_commit_hash: Option<HashId>,
        store: &mut ContextKeyValueStore,
        commit_to_storage: bool,
    ) -> Result<PostCommitData, MerkleError> {
        let root_hash = self.get_root_directory_hash(store)?;
        let root = self.get_root_directory();

        let new_commit = Commit {
            parent_commit_hash,
            root_hash,
            time,
            author,
            message,
        };
        let object = Object::Commit(Box::new(new_commit.clone()));
        let commit_hash = hash_commit(&new_commit, store)?;

        // produce objects to be persisted to storage
        let mut data = SerializingData::new(store);
        if commit_to_storage {
            let storage = self.index.storage.borrow();
            self.serialize_objects_recursively(
                &object,
                commit_hash,
                Some(root),
                &mut data,
                &storage,
            )?;
        }

        Ok(PostCommitData {
            commit_hash_id: commit_hash,
            batch: data.batch,
            reused: data.referenced_older_objects,
            serialize_stats: data.stats,
        })
    }

    /// Returns a new version of the WorkingTree with the tree replaced
    pub fn with_new_root(&self, dir_id: DirectoryId) -> Self {
        Self::new_with_directory(self.index.clone(), dir_id)
    }

    /// Set key/val to the working tree.
    pub fn add(&self, key: &ContextKey, value: &[u8]) -> Result<Self, MerkleError> {
        let mut storage = self.index.storage.borrow_mut();
        let blob_id = storage.add_blob_by_ref(value)?;

        let dir_entry = DirEntry::new_blob(Object::Blob(blob_id));
        let object = &self._add(key, dir_entry, &mut storage)?;
        let dir_id = self.object_directory(object)?;

        Ok(self.with_new_root(dir_id))
    }

    fn _add(
        &self,
        key: &ContextKey,
        dir_entry: DirEntry,
        storage: &mut Storage,
    ) -> Result<Object, MerkleError> {
        self.compute_new_root_with_change(&key, Some(dir_entry), storage)
    }

    /// Delete an item from the staging area.
    pub fn delete(&self, key: &ContextKey) -> Result<Self, MerkleError> {
        let new_root_object = &self._delete(key)?;
        let dir_id = self.object_directory(new_root_object)?;
        Ok(self.with_new_root(dir_id))
    }

    fn _delete(&self, key: &ContextKey) -> Result<Object, MerkleError> {
        let root = self.get_root_directory();

        if key.is_empty() {
            return Ok(Object::Directory(root));
        }
        let mut storage = self.index.storage.borrow_mut();
        self.compute_new_root_with_change(&key, None, &mut storage)
    }

    /// Get a new directory with `new_dir_entry` put under given `key`.
    /// Walk down the directory to find key, set new value and walk back up recreating the tree
    ///
    /// # Arguments
    ///
    /// * `root` - DirectoryId to modify
    /// * `key` - path under which the changes takes place
    /// * `new_dir_entry` - None for deletion, Some for inserting a hash under the key.
    fn compute_new_root_with_change(
        &self,
        key: &[&str],
        new_dir_entry: Option<DirEntry>,
        storage: &mut Storage,
    ) -> Result<Object, MerkleError> {
        let last = match key.last() {
            Some(last) => *last,
            None => match new_dir_entry {
                Some(n) => {
                    // if there is a value we want to assigin - just
                    // assigin it
                    return n
                        .get_object()
                        .ok_or(MerkleError::InvalidState("Missing object value"));
                }
                None => {
                    // if key is empty and there is new_dir_entry == None
                    // that means that we just removed whole tree
                    // so set merkle storage root to empty dir and place
                    // it in staging area
                    return Ok(Object::Directory(DirectoryId::empty()));
                }
            },
        };

        let path = &key[..key.len() - 1];
        let root = self.get_root_directory();
        let dir_id = self.find_or_create_directory(root, path, storage)?;

        // If this was a deletion, and the path doesn't contain anything
        // there is nothing to do. We don't want to recurse in this case.
        if dir_id.is_empty() && new_dir_entry.is_none() {
            return Ok(Object::Directory(self.get_root_directory()));
        }

        let dir_id = match new_dir_entry {
            None => storage.dir_remove(dir_id, last)?,
            Some(new_dir_entry) => storage.dir_insert(dir_id, last, new_dir_entry)?,
        };

        if dir_id.is_empty() {
            self.compute_new_root_with_change(path, None, storage)
        } else {
            self.compute_new_root_with_change(
                path,
                Some(DirEntry::new_directory(Object::Directory(dir_id))),
                storage,
            )
        }
    }

    /// Find directory by path and return it.
    ///
    /// Fetches data from the repository if necessary,
    /// Return an empty directory if no directory under this path exists or if a blob
    /// (= value) is encountered along the way.
    fn find_or_create_directory(
        &self,
        root: DirectoryId,
        key: &ContextKey,
        storage: &mut Storage,
    ) -> Result<DirectoryId, MerkleError> {
        self.index.find_or_create_directory(root, key, storage)
    }

    /// Returns the root hash of this working tree.
    ///
    /// Note that this methods considers root value (blob) as an empty directory.
    /// Use `Self::hash` to get the correct hash in all cases (when the root is
    /// a value or directory).
    pub fn get_root_directory_hash(
        &self,
        store: &mut ContextKeyValueStore,
    ) -> Result<HashId, MerkleError> {
        // TOOD: unnecessery recalculation, should be one when set_staged_root
        let root = self.get_root_directory();
        let storage = self.index.storage.borrow();
        hash_directory(root, store, &storage).map_err(MerkleError::from)
    }

    /// Serializes working tree and builds vector of objects to be persisted to DB, recursively.
    ///
    /// This doesn't traverse objects that were already commited.
    fn serialize_objects_recursively(
        &self,
        object: &Object,
        object_hash: HashId,
        root: Option<DirectoryId>,
        data: &mut SerializingData,
        storage: &Storage,
    ) -> Result<(), MerkleError> {
        // Add object to batch
        data.add_serialized_object(object_hash, object, storage)?;

        match object {
            Object::Blob(_blob_id) => Ok(()),
            Object::Directory(dir_id) => {
                storage.dir_iterate_unsorted(*dir_id, |&(_, dir_entry_id)| {
                    let child_dir_entry = storage.get_dir_entry(dir_entry_id)?;

                    let object_hash = match child_dir_entry.object_hash_id(data.store, storage)? {
                        Some(hash_id) => hash_id,
                        None => return Ok(()), // Object is an inlined blob, we don't serialize them.
                    };

                    if child_dir_entry.is_commited() {
                        data.add_older_object(child_dir_entry, storage)?;
                        return Ok(());
                    }
                    child_dir_entry.set_commited(true);

                    match child_dir_entry.get_object().as_ref() {
                        None => Ok(()),
                        Some(object) => self.serialize_objects_recursively(
                            object,
                            object_hash,
                            None,
                            data,
                            storage,
                        ),
                    }
                })
            }
            Object::Commit(commit) => {
                let object = match root {
                    Some(root) => Object::Directory(root),
                    None => self.fetch_object_from_repo(commit.root_hash, data.store)?,
                };
                self.serialize_objects_recursively(&object, commit.root_hash, None, data, storage)
            }
        }
    }

    /// Fetches object of this `hash_id` from the repository.
    fn fetch_object_from_repo(
        &self,
        hash_id: HashId,
        store: &ContextKeyValueStore,
    ) -> Result<Object, MerkleError> {
        match store.get_value(hash_id)? {
            None => Err(MerkleError::ObjectNotFound { hash_id }),
            Some(object_bytes) => {
                let mut storage = self.index.storage.borrow_mut();
                deserialize(object_bytes.as_ref(), &mut storage, store).map_err(Into::into)
            }
        }
    }

    /// Extracts the directory of this object.
    ///
    /// Returns an error when it's not a directory.
    fn object_directory(&self, object: &Object) -> Result<DirectoryId, MerkleError> {
        match object {
            Object::Directory(dir_id) => Ok(*dir_id),
            Object::Blob(_) => Err(MerkleError::FoundUnexpectedStructure {
                sought: "directory".to_string(),
                found: "blob".to_string(),
            }),
            Object::Commit { .. } => Err(MerkleError::FoundUnexpectedStructure {
                sought: "directory".to_string(),
                found: "commit".to_string(),
            }),
        }
    }

    /// Extracts the value (blob) of this object.
    ///
    /// Returns an error when it's not a value.
    fn object_value(&self, object: &Object) -> Result<BlobId, MerkleError> {
        match object {
            Object::Blob(blob_id) => Ok(*blob_id),
            Object::Directory(_) => Err(MerkleError::FoundUnexpectedStructure {
                sought: "blob".to_string(),
                found: "directory".to_string(),
            }),
            Object::Commit { .. } => Err(MerkleError::FoundUnexpectedStructure {
                sought: "blob".to_string(),
                found: "commit".to_string(),
            }),
        }
    }

    /// Returns the object of this `dir_entry_id`.
    ///
    /// Fetch it from the repository when not available in `Self::storage`.
    fn dir_entry_object(
        &self,
        dir_entry_id: DirEntryId,
        storage: &mut Storage,
    ) -> Result<Object, MerkleError> {
        self.index.dir_entry_object(dir_entry_id, storage)
    }

    /// Returns the root of this working tree.
    ///
    /// Returns an empty directory when the root is a value (blob).
    fn get_root_directory(&self) -> DirectoryId {
        match self.root {
            WorkingTreeRoot::Directory(dir_id) => dir_id,
            WorkingTreeRoot::Value(_) => DirectoryId::empty(),
        }
    }

    // TODO: only used in tests on this file
    //    pub fn get_staged_entries(&self) -> Result<std::string::String, MerkleError> {
    //        let mut result = String::new();
    //        for (hash, entry) in self.staged_cache.get_mut().cached() {
    //            match entry {
    //                Entry::Blob(blob) => {
    //                    result += &format!("{}: Value {:?}, \n", hex::encode(&hash[0..3]), blob);
    //                }
    //
    //                Entry::Tree(tree) => {
    //                    if tree.is_empty() {
    //                        continue;
    //                    }
    //                    let tree_hash = &hash_tree(tree)?[0..3];
    //                    result += &format!("{}: Tree {{", hex::encode(tree_hash));
    //
    //                    for (path, val) in tree {
    //                        let kind = if let DirEntryKind::Directory = val.dir_entry_kind {
    //                            "Tree"
    //                        } else {
    //                            "Value/Blob"
    //                        };
    //                        result += &format!(
    //                            "{}: {}({:?}), ",
    //                            path,
    //                            kind,
    //                            hex::encode(&val.entry_hash[0..3])
    //                        );
    //                    }
    //                    result += "}}\n";
    //                }
    //
    //                Entry::Commit(_) => {
    //                    return Err(MerkleError::InvalidState(
    //                        "commits must not occur in staged area",
    //                    ));
    //                }
    //            }
    //        }
    //        Ok(result)
    //    }
}

//* /// Merkle storage predefined tests with abstraction for underlaying kv_store for context
//*#[cfg(test)]
//*mod tests {
//*    use std::env;
//*    use std::path::PathBuf;
//*
//*    use assert_json_diff::assert_json_eq;
//*
//*    use crate::context::kv_store::test_support::TestContextKvStoreFactoryInstance;
//*    use crate::context::kv_store::SupportedContextKeyValueStore;
//*    use crate::context::ContextValue;
//*
//*    use super::*;
//*
//*    fn get_short_hash(hash: &EntryHash) -> String {
//*        hex::encode(&hash[0..3])
//*    }
//*
//*    fn get_staged_root_short_hash(storage: &mut WorkingTree) -> Result<String, MerkleError> {
//*        let hash = storage.get_staged_root_hash()?;
//*        Ok(get_short_hash(&hash))
//*    }
//*
//*    fn test_duplicate_entry_in_staging(kv_store_factory: &TestContextKvStoreFactoryInstance) {
//*        let mut storage = WorkingTree::new(
//*            kv_store_factory
//*                .create("test_duplicate_entry_in_staging")
//*                .unwrap(),
//*        );
//*
//*        let a_foo: &ContextKey = &vec!["a".to_string(), "foo".to_string()];
//*        let c_foo: &ContextKey = &vec!["c".to_string(), "foo".to_string()];
//*        storage
//*            .set(1, &vec!["a".to_string(), "foo".to_string()], vec![97, 98])
//*            .unwrap();
//*        storage
//*            .set(2, &vec!["c".to_string(), "zoo".to_string()], vec![1, 2])
//*            .unwrap();
//*        storage
//*            .set(3, &vec!["c".to_string(), "foo".to_string()], vec![97, 98])
//*            .unwrap();
//*        storage
//*            .delete(4, &vec!["c".to_string(), "zoo".to_string()])
//*            .unwrap();
//*        // now c/ is the same tree as a/ - which means there are two references to single entry in staging area
//*        // modify the tree and check that the other one was kept intact
//*        storage
//*            .set(5, &vec!["c".to_string(), "foo".to_string()], vec![3, 4])
//*            .unwrap();
//*        let commit = storage
//*            .commit(0, "Tezos".to_string(), "Genesis".to_string())
//*            .unwrap();
//*        assert_eq!(storage.get_history(&commit, a_foo).unwrap(), vec![97, 98]);
//*        assert_eq!(storage.get_history(&commit, c_foo).unwrap(), vec![3, 4]);
//*    }
//*
//*    fn test_tree_hash(kv_store_factory: &TestContextKvStoreFactoryInstance) {
//*        let mut storage = WorkingTree::new(kv_store_factory.create("test_tree_hash").unwrap());
//*
//*        storage
//*            .set(
//*                1,
//*                &vec!["a".to_string(), "foo".to_string()],
//*                vec![97, 98, 99],
//*            )
//*            .unwrap(); // abc
//*        storage
//*            .set(2, &vec!["b".to_string(), "boo".to_string()], vec![97, 98])
//*            .unwrap();
//*        storage
//*            .set(
//*                3,
//*                &vec!["a".to_string(), "aaa".to_string()],
//*                vec![97, 98, 99, 100],
//*            )
//*            .unwrap();
//*        storage.set(4, &vec!["x".to_string()], vec![97]).unwrap();
//*        storage
//*            .set(
//*                5,
//*                &vec!["one".to_string(), "two".to_string(), "three".to_string()],
//*                vec![97],
//*            )
//*            .unwrap();
//*        storage
//*            .commit(0, "Tezos".to_string(), "Genesis".to_string())
//*            .unwrap();
//*
//*        let hash = storage.get_staged_root_hash().unwrap();
//*
//*        assert_eq!([0xDB, 0xAE, 0xD7, 0xB6], hash[0..4]);
//*    }
//*
//*    fn test_commit_hash(kv_store_factory: &TestContextKvStoreFactoryInstance) {
//*        let mut storage = WorkingTree::new(kv_store_factory.create("test_commit_hash").unwrap());
//*
//*        storage
//*            .set(1, &vec!["a".to_string()], vec![97, 98, 99])
//*            .unwrap();
//*
//*        let commit = storage.commit(0, "Tezos".to_string(), "Genesis".to_string());
//*
//*        assert_eq!([0xCF, 0x95, 0x18, 0x33], commit.unwrap()[0..4]);
//*
//*        storage
//*            .set(1, &vec!["data".to_string(), "x".to_string()], vec![97])
//*            .unwrap();
//*        let commit = storage.commit(0, "Tezos".to_string(), "".to_string());
//*
//*        assert_eq!([0xCA, 0x7B, 0xC7, 0x02], commit.unwrap()[0..4]);
//*        // full irmin hash: ca7bc7022ffbd35acc97f7defb00c486bb7f4d19a2d62790d5949775eb74f3c8
//*    }
//*
//*    fn test_examples_from_article_about_storage(
//*        kv_store_factory: &TestContextKvStoreFactoryInstance,
//*    ) {
//*        let mut storage = WorkingTree::new(
//*            kv_store_factory
//*                .create("test_examples_from_article_about_storage")
//*                .unwrap(),
//*        );
//*
//*        storage.set(1, &vec!["a".to_string()], vec![1]).unwrap();
//*        let root = get_staged_root_short_hash(&mut storage).expect("hash error");
//*        println!("SET [a] = 1\nROOT: {}", root);
//*        println!("CONTENT {}", storage.get_staged_entries().unwrap());
//*        assert_eq!(root, "d49a53".to_string());
//*
//*        storage
//*            .set(2, &vec!["b".to_string(), "c".to_string()], vec![1])
//*            .unwrap();
//*        let root = get_staged_root_short_hash(&mut storage).expect("hash error");
//*        println!("\nSET [b,c] = 1\nROOT: {}", root);
//*        print!("{}", storage.get_staged_entries().unwrap());
//*        assert_eq!(root, "ed8adf".to_string());
//*
//*        storage
//*            .set(3, &vec!["b".to_string(), "d".to_string()], vec![2])
//*            .unwrap();
//*        let root = get_staged_root_short_hash(&mut storage).expect("hash error");
//*        println!("\nSET [b,d] = 2\nROOT: {}", root);
//*        print!("{}", storage.get_staged_entries().unwrap());
//*        assert_eq!(root, "437186".to_string());
//*
//*        storage.set(4, &vec!["a".to_string()], vec![2]).unwrap();
//*        let root = get_staged_root_short_hash(&mut storage).expect("hash error");
//*        println!("\nSET [a] = 2\nROOT: {}", root);
//*        print!("{}", storage.get_staged_entries().unwrap());
//*        assert_eq!(root, "0d78b3".to_string());
//*
//*        let entries = storage.get_staged_entries().unwrap();
//*        let commit_hash = storage
//*            .commit(0, "Tezedge".to_string(), "persist changes".to_string())
//*            .unwrap();
//*        println!("\nCOMMIT time:0 author:'tezedge' message:'persist'");
//*        println!("ROOT: {}", get_short_hash(&commit_hash));
//*        if let Entry::Commit(c) = storage.get_entry(&commit_hash).unwrap() {
//*            println!("{} : Commit{{time:{}, message:{}, author:{}, root_hash:{}, parent_commit_hash: None}}", get_short_hash(&commit_hash), c.time, c.message, c.author, get_short_hash(&c.root_hash));
//*        }
//*        print!("{}", entries);
//*        assert_eq!("e6de3f", get_short_hash(&commit_hash))
//*    }
//*
//*    fn test_multiple_commit_hash(kv_store_factory: &TestContextKvStoreFactoryInstance) {
//*        let mut storage = WorkingTree::new(
//*            kv_store_factory
//*                .create("test_multiple_commit_hash")
//*                .unwrap(),
//*        );
//*
//*        let _commit = storage.commit(0, "Tezos".to_string(), "Genesis".to_string());
//*
//*        storage
//*            .set(
//*                1,
//*                &vec!["data".to_string(), "a".to_string(), "x".to_string()],
//*                vec![97],
//*            )
//*            .unwrap();
//*        storage
//*            .copy(
//*                2,
//*                &vec!["data".to_string(), "a".to_string()],
//*                &vec!["data".to_string(), "b".to_string()],
//*            )
//*            .unwrap();
//*        storage
//*            .delete(
//*                3,
//*                &vec!["data".to_string(), "b".to_string(), "x".to_string()],
//*            )
//*            .unwrap();
//*        let commit = storage.commit(0, "Tezos".to_string(), "".to_string());
//*
//*        assert_eq!([0x9B, 0xB0, 0x0D, 0x6E], commit.unwrap()[0..4]);
//*    }
//*
//*    fn test_get(kv_store_factory: &TestContextKvStoreFactoryInstance) {
//*        let db_name = "test_get";
//*
//*        let key_abc: &ContextKey = &vec!["a".to_string(), "b".to_string(), "c".to_string()];
//*        let key_abx: &ContextKey = &vec!["a".to_string(), "b".to_string(), "x".to_string()];
//*        let key_eab: &ContextKey = &vec!["e".to_string(), "a".to_string(), "b".to_string()];
//*        let key_az: &ContextKey = &vec!["a".to_string(), "z".to_string()];
//*        let key_d: &ContextKey = &vec!["d".to_string()];
//*
//*        let kv_store = kv_store_factory.create(db_name).unwrap();
//*        let mut storage = WorkingTree::new(kv_store);
//*
//*        let res = storage.get(&vec![]);
//*        assert_eq!(res.unwrap().is_empty(), true);
//*        let res = storage.get(&vec!["a".to_string()]);
//*        assert_eq!(res.unwrap().is_empty(), true);
//*
//*        storage.set(1, key_abc, vec![1u8, 2u8]).unwrap();
//*        storage.set(2, key_abx, vec![3u8]).unwrap();
//*        assert_eq!(storage.get(&key_abc).unwrap(), vec![1u8, 2u8]);
//*        assert_eq!(storage.get(&key_abx).unwrap(), vec![3u8]);
//*        let commit1 = storage.commit(0, "".to_string(), "".to_string()).unwrap();
//*
//*        storage.set(3, key_az, vec![4u8]).unwrap();
//*        storage.set(4, key_abx, vec![5u8]).unwrap();
//*        storage.set(5, key_d, vec![6u8]).unwrap();
//*        storage.set(6, key_eab, vec![7u8]).unwrap();
//*        assert_eq!(storage.get(key_az).unwrap(), vec![4u8]);
//*        assert_eq!(storage.get(key_abx).unwrap(), vec![5u8]);
//*        assert_eq!(storage.get(key_d).unwrap(), vec![6u8]);
//*        assert_eq!(storage.get(key_eab).unwrap(), vec![7u8]);
//*        let commit2 = storage.commit(0, "".to_string(), "".to_string()).unwrap();
//*
//*        assert_eq!(
//*            storage.get_history(&commit1, key_abc).unwrap(),
//*            vec![1u8, 2u8]
//*        );
//*        assert_eq!(storage.get_history(&commit1, key_abx).unwrap(), vec![3u8]);
//*        assert_eq!(storage.get_history(&commit2, key_abx).unwrap(), vec![5u8]);
//*        assert_eq!(storage.get_history(&commit2, key_az).unwrap(), vec![4u8]);
//*        assert_eq!(storage.get_history(&commit2, key_d).unwrap(), vec![6u8]);
//*        assert_eq!(storage.get_history(&commit2, key_eab).unwrap(), vec![7u8]);
//*    }
//*
//*    fn test_mem(kv_store_factory: &TestContextKvStoreFactoryInstance) {
//*        let mut storage = WorkingTree::new(kv_store_factory.create("test_mem").unwrap());
//*
//*        let key_abc: &ContextKey = &vec!["a".to_string(), "b".to_string(), "c".to_string()];
//*        let key_abx: &ContextKey = &vec!["a".to_string(), "b".to_string(), "x".to_string()];
//*
//*        assert_eq!(storage.mem(&key_abc).unwrap(), false);
//*        assert_eq!(storage.mem(&key_abx).unwrap(), false);
//*        storage.set(1, key_abc, vec![1u8, 2u8]).unwrap();
//*        assert_eq!(storage.mem(&key_abc).unwrap(), true);
//*        assert_eq!(storage.mem(&key_abx).unwrap(), false);
//*        storage.set(2, key_abx, vec![3u8]).unwrap();
//*        assert_eq!(storage.mem(&key_abc).unwrap(), true);
//*        assert_eq!(storage.mem(&key_abx).unwrap(), true);
//*        storage.delete(3, key_abx).unwrap();
//*        assert_eq!(storage.mem(&key_abc).unwrap(), true);
//*        assert_eq!(storage.mem(&key_abx).unwrap(), false);
//*    }
//*
//*    fn test_dirmem(kv_store_factory: &TestContextKvStoreFactoryInstance) {
//*        let mut storage = WorkingTree::new(kv_store_factory.create("test_dirmem").unwrap());
//*
//*        let key_abc: &ContextKey = &vec!["a".to_string(), "b".to_string(), "c".to_string()];
//*        let key_ab: &ContextKey = &vec!["a".to_string(), "b".to_string()];
//*        let key_a: &ContextKey = &vec!["a".to_string()];
//*
//*        assert_eq!(storage.dirmem(&key_a).unwrap(), false);
//*        assert_eq!(storage.dirmem(&key_ab).unwrap(), false);
//*        assert_eq!(storage.dirmem(&key_abc).unwrap(), false);
//*        storage.set(1, key_abc, vec![1u8, 2u8]).unwrap();
//*        assert_eq!(storage.dirmem(&key_a).unwrap(), true);
//*        assert_eq!(storage.dirmem(&key_ab).unwrap(), true);
//*        assert_eq!(storage.dirmem(&key_abc).unwrap(), false);
//*        storage.delete(2, key_abc).unwrap();
//*        assert_eq!(storage.dirmem(&key_a).unwrap(), false);
//*        assert_eq!(storage.dirmem(&key_ab).unwrap(), false);
//*        assert_eq!(storage.dirmem(&key_abc).unwrap(), false);
//*    }
//*
//*    fn test_copy(kv_store_factory: &TestContextKvStoreFactoryInstance) {
//*        let mut storage = WorkingTree::new(kv_store_factory.create("test_copy").unwrap());
//*
//*        let key_abc: &ContextKey = &vec!["a".to_string(), "b".to_string(), "c".to_string()];
//*        storage.set(1, key_abc, vec![1_u8]).unwrap();
//*        storage
//*            .copy(2, &vec!["a".to_string()], &vec!["z".to_string()])
//*            .unwrap();
//*
//*        assert_eq!(
//*            vec![1_u8],
//*            storage
//*                .get(&vec!["z".to_string(), "b".to_string(), "c".to_string()])
//*                .unwrap()
//*        );
//*        // TODO test copy over commits
//*    }
//*
//*    fn test_delete(kv_store_factory: &TestContextKvStoreFactoryInstance) {
//*        let mut storage = WorkingTree::new(kv_store_factory.create("test_delete").unwrap());
//*
//*        let key_abc: &ContextKey = &vec!["a".to_string(), "b".to_string(), "c".to_string()];
//*        let key_abx: &ContextKey = &vec!["a".to_string(), "b".to_string(), "x".to_string()];
//*        storage.set(1, key_abc, vec![2_u8]).unwrap();
//*        storage.set(2, key_abx, vec![3_u8]).unwrap();
//*        storage.delete(3, key_abx).unwrap();
//*        let commit1 = storage.commit(0, "".to_string(), "".to_string()).unwrap();
//*
//*        assert!(storage.get_history(&commit1, &key_abx).is_err());
//*    }
//*
//*    fn test_deleted_entry_available(kv_store_factory: &TestContextKvStoreFactoryInstance) {
//*        let mut storage = WorkingTree::new(
//*            kv_store_factory
//*                .create("test_deleted_entry_available")
//*                .unwrap(),
//*        );
//*
//*        let key_abc: &ContextKey = &vec!["a".to_string(), "b".to_string(), "c".to_string()];
//*        storage.set(1, key_abc, vec![2_u8]).unwrap();
//*        let commit1 = storage.commit(0, "".to_string(), "".to_string()).unwrap();
//*        storage.delete(2, key_abc).unwrap();
//*        let _commit2 = storage.commit(0, "".to_string(), "".to_string()).unwrap();
//*
//*        assert_eq!(vec![2_u8], storage.get_history(&commit1, &key_abc).unwrap());
//*    }
//*
//*    fn test_delete_in_separate_commit(kv_store_factory: &TestContextKvStoreFactoryInstance) {
//*        let mut storage = WorkingTree::new(
//*            kv_store_factory
//*                .create("test_delete_in_separate_commit")
//*                .unwrap(),
//*        );
//*
//*        let key_abc: &ContextKey = &vec!["a".to_string(), "b".to_string(), "c".to_string()];
//*        let key_abx: &ContextKey = &vec!["a".to_string(), "b".to_string(), "x".to_string()];
//*        storage.set(1, key_abc, vec![2_u8]).unwrap();
//*        storage.set(2, key_abx, vec![3_u8]).unwrap();
//*        storage.commit(0, "".to_string(), "".to_string()).unwrap();
//*
//*        storage.delete(1, key_abx).unwrap();
//*        let commit2 = storage.commit(0, "".to_string(), "".to_string()).unwrap();
//*
//*        assert!(storage.get_history(&commit2, &key_abx).is_err());
//*    }
//*
//*    fn test_checkout(kv_store_factory: &TestContextKvStoreFactoryInstance) {
//*        let key_abc: &ContextKey = &vec!["a".to_string(), "b".to_string(), "c".to_string()];
//*        let key_abx: &ContextKey = &vec!["a".to_string(), "b".to_string(), "x".to_string()];
//*
//*        let mut storage = WorkingTree::new(kv_store_factory.create("test_checkout").unwrap());
//*
//*        storage.set(1, key_abc, vec![1u8]).unwrap();
//*        storage.set(2, key_abx, vec![2u8]).unwrap();
//*        let commit1 = storage.commit(0, "".to_string(), "".to_string()).unwrap();
//*
//*        storage.set(1, key_abc, vec![3u8]).unwrap();
//*        storage.set(2, key_abx, vec![4u8]).unwrap();
//*        let commit2 = storage.commit(0, "".to_string(), "".to_string()).unwrap();
//*
//*        storage.checkout(&commit1).unwrap();
//*        assert_eq!(storage.get(&key_abc).unwrap(), vec![1u8]);
//*        assert_eq!(storage.get(&key_abx).unwrap(), vec![2u8]);
//*        // this set be wiped by checkout
//*        storage.set(1, key_abc, vec![8u8]).unwrap();
//*
//*        storage.checkout(&commit2).unwrap();
//*        assert_eq!(storage.get(&key_abc).unwrap(), vec![3u8]);
//*        assert_eq!(storage.get(&key_abx).unwrap(), vec![4u8]);
//*    }
//*
//*    /// Test getting entire tree in string format for JSON RPC
//*    fn test_get_context_tree_by_prefix(kv_store_factory: &TestContextKvStoreFactoryInstance) {
//*        let mut storage = WorkingTree::new(
//*            kv_store_factory
//*                .create("test_get_context_tree_by_prefix")
//*                .unwrap(),
//*        );
//*
//*        let all_json = serde_json::json!(
//*            {
//*                "adata": {
//*                    "b": {
//*                            "x": {
//*                                    "y":"090a"
//*                            }
//*                    }
//*                },
//*                "data": {
//*                    "a": {
//*                            "x": {
//*                                    "y":"0506"
//*                            }
//*                    },
//*                    "b": {
//*                            "x": {
//*                                    "y":"0708"
//*                            }
//*                    },
//*                    "c":"0102"
//*                }
//*            }
//*        );
//*        let data_json = serde_json::json!(
//*            {
//*                "a": {
//*                        "x": {
//*                                "y":"0506"
//*                        }
//*                },
//*                "b": {
//*                        "x": {
//*                                "y":"0708"
//*                        }
//*                },
//*                "c":"0102"
//*            }
//*        );
//*
//*        let _commit = storage
//*            .commit(0, "Tezos".to_string(), "Genesis".to_string())
//*            .unwrap();
//*
//*        storage
//*            .set(
//*                1,
//*                &vec!["data".to_string(), "a".to_string(), "x".to_string()],
//*                vec![3, 4],
//*            )
//*            .unwrap();
//*        storage
//*            .set(2, &vec!["data".to_string(), "a".to_string()], vec![1, 2])
//*            .unwrap();
//*        storage
//*            .set(
//*                3,
//*                &vec![
//*                    "data".to_string(),
//*                    "a".to_string(),
//*                    "x".to_string(),
//*                    "y".to_string(),
//*                ],
//*                vec![5, 6],
//*            )
//*            .unwrap();
//*        storage
//*            .set(
//*                4,
//*                &vec![
//*                    "data".to_string(),
//*                    "b".to_string(),
//*                    "x".to_string(),
//*                    "y".to_string(),
//*                ],
//*                vec![7, 8],
//*            )
//*            .unwrap();
//*        storage
//*            .set(5, &vec!["data".to_string(), "c".to_string()], vec![1, 2])
//*            .unwrap();
//*        storage
//*            .set(
//*                6,
//*                &vec![
//*                    "adata".to_string(),
//*                    "b".to_string(),
//*                    "x".to_string(),
//*                    "y".to_string(),
//*                ],
//*                vec![9, 10],
//*            )
//*            .unwrap();
//*        //data-a[1,2]
//*        //data-a-x[3,4]
//*        //data-a-x-y[5,6]
//*        //data-b-x-y[7,8]
//*        //data-c[1,2]
//*        //adata-b-x-y[9,10]
//*        let commit = storage.commit(0, "Tezos".to_string(), "Genesis".to_string());
//*
//*        // without depth
//*        let rv_all = storage
//*            .get_context_tree_by_prefix(&commit.as_ref().unwrap(), &vec![], None)
//*            .unwrap();
//*        assert_json_eq!(all_json, serde_json::to_value(&rv_all).unwrap());
//*
//*        let rv_data = storage
//*            .get_context_tree_by_prefix(&commit.as_ref().unwrap(), &vec!["data".to_string()], None)
//*            .unwrap();
//*        assert_json_eq!(data_json, serde_json::to_value(&rv_data).unwrap());
//*
//*        // with depth 0
//*        assert_json_eq!(
//*            serde_json::json!(null),
//*            serde_json::to_value(
//*                storage
//*                    .get_context_tree_by_prefix(&commit.as_ref().unwrap(), &vec![], Some(0))
//*                    .unwrap()
//*            )
//*            .unwrap()
//*        );
//*
//*        // with depth 1
//*        assert_json_eq!(
//*            serde_json::json!(
//*                {
//*                    "adata": null,
//*                    "data": null
//*                }
//*            ),
//*            serde_json::to_value(
//*                storage
//*                    .get_context_tree_by_prefix(&commit.as_ref().unwrap(), &vec![], Some(1))
//*                    .unwrap()
//*            )
//*            .unwrap()
//*        );
//*        // with depth 2
//*        assert_json_eq!(
//*            serde_json::json!(
//*                {
//*                    "adata": {
//*                        "b" : null
//*                    },
//*                    "data": {
//*                        "a" : null,
//*                        "b" : null,
//*                        "c" : null,
//*                    },
//*                }
//*            ),
//*            serde_json::to_value(
//*                storage
//*                    .get_context_tree_by_prefix(&commit.as_ref().unwrap(), &vec![], Some(2))
//*                    .unwrap()
//*            )
//*            .unwrap()
//*        );
//*    }
//*
//*    fn test_backtracking_on_set(kv_store_factory: &TestContextKvStoreFactoryInstance) {
//*        let mut storage =
//*            WorkingTree::new(kv_store_factory.create("test_backtracking_on_set").unwrap());
//*
//*        let dummy_key = &vec!["a".to_string()];
//*        storage.set(1, dummy_key, vec![1u8]).unwrap();
//*        storage.set(2, dummy_key, vec![2u8]).unwrap();
//*
//*        // get recent value
//*        assert_eq!(storage.get(dummy_key).unwrap(), vec![2u8]);
//*
//*        // checkout previous stage state
//*        storage.stage_checkout(1).unwrap();
//*        assert_eq!(storage.get(dummy_key).unwrap(), vec![1u8]);
//*
//*        // checkout newest stage state
//*        storage.stage_checkout(2).unwrap();
//*        assert_eq!(storage.get(dummy_key).unwrap(), vec![2u8]);
//*    }
//*
//*    fn test_backtracking_on_delete(kv_store_factory: &TestContextKvStoreFactoryInstance) {
//*        let mut storage = WorkingTree::new(
//*            kv_store_factory
//*                .create("test_backtracking_on_delete")
//*                .unwrap(),
//*        );
//*
//*        let key = &vec!["a".to_string()];
//*        let value = vec![1u8];
//*        let empty_response: ContextValue = Vec::new();
//*
//*        storage.set(1, key, value.clone()).unwrap();
//*        storage.delete(2, key).unwrap();
//*
//*        assert_eq!(storage.get(key).unwrap(), empty_response);
//*
//*        // // checkout previous stage state
//*        storage.stage_checkout(1).unwrap();
//*        assert_eq!(storage.get(key).unwrap(), value);
//*
//*        // checkout latest stage state
//*        storage.stage_checkout(2).unwrap();
//*        assert_eq!(storage.get(key).unwrap(), empty_response);
//*    }
//*
//*    // Currently we don't perform a cleanup after each COMMIT
//*    // That will happen during the next CHECKOUT, this test is to ensure that
//*    fn test_checkout_stage_from_before_commit(
//*        kv_store_factory: &TestContextKvStoreFactoryInstance,
//*    ) {
//*        let mut storage = WorkingTree::new(
//*            kv_store_factory
//*                .create("test_checkout_stage_from_before_commit")
//*                .unwrap(),
//*        );
//*
//*        let key = &vec!["a".to_string()];
//*        storage.set(1, key, vec![1u8]).unwrap();
//*        storage.set(2, key, vec![2u8]).unwrap();
//*        storage
//*            .commit(0, "author".to_string(), "message".to_string())
//*            .unwrap();
//*
//*        assert_eq!(storage.staged.is_empty(), false);
//*        assert_eq!(storage.stage_checkout(1).is_err(), false);
//*    }
//*
//*    macro_rules! tests_with_storage {
//*        ($storage_tests_name:ident, $kv_store_factory:expr) => {
//*            mod $storage_tests_name {
//*                #[test]
//*                fn test_tree_hash() {
//*                    super::test_tree_hash($kv_store_factory)
//*                }
//*                #[test]
//*                fn test_duplicate_entry_in_staging() {
//*                    super::test_duplicate_entry_in_staging($kv_store_factory)
//*                }
//*                #[test]
//*                fn test_commit_hash() {
//*                    super::test_commit_hash($kv_store_factory)
//*                }
//*                #[test]
//*                fn test_examples_from_article_about_storage() {
//*                    super::test_examples_from_article_about_storage($kv_store_factory)
//*                }
//*                #[test]
//*                fn test_multiple_commit_hash() {
//*                    super::test_multiple_commit_hash($kv_store_factory)
//*                }
//*                #[test]
//*                fn test_get() {
//*                    super::test_get($kv_store_factory)
//*                }
//*                #[test]
//*                fn test_mem() {
//*                    super::test_mem($kv_store_factory)
//*                }
//*                #[test]
//*                fn test_dirmem() {
//*                    super::test_dirmem($kv_store_factory)
//*                }
//*                #[test]
//*                fn test_copy() {
//*                    super::test_copy($kv_store_factory)
//*                }
//*                #[test]
//*                fn test_delete() {
//*                    super::test_delete($kv_store_factory)
//*                }
//*                #[test]
//*                fn test_deleted_entry_available() {
//*                    super::test_deleted_entry_available($kv_store_factory)
//*                }
//*                #[test]
//*                fn test_delete_in_separate_commit() {
//*                    super::test_delete_in_separate_commit($kv_store_factory)
//*                }
//*                #[test]
//*                fn test_checkout() {
//*                    super::test_checkout($kv_store_factory)
//*                }
//*                #[test]
//*                fn test_get_context_tree_by_prefix() {
//*                    super::test_get_context_tree_by_prefix($kv_store_factory)
//*                }
//*                #[test]
//*                fn test_backtracking_on_set() {
//*                    super::test_backtracking_on_set($kv_store_factory)
//*                }
//*                #[test]
//*                fn test_backtracking_on_delete() {
//*                    super::test_backtracking_on_delete($kv_store_factory)
//*                }
//*                #[test]
//*                fn test_fail_to_checkout_stage_from_before_commit() {
//*                    super::test_checkout_stage_from_before_commit($kv_store_factory)
//*                }
//*            }
//*        };
//*    }
//*
//*    lazy_static::lazy_static! {
//*        static ref SUPPORTED_KV_STORES: std::collections::HashMap<SupportedContextKeyValueStore, TestContextKvStoreFactoryInstance> = crate::context::kv_store::test_support::all_kv_stores(out_dir_path());
//*    }
//*
//*    fn out_dir_path() -> PathBuf {
//*        let out_dir = env::var("OUT_DIR").expect(
//*            "OUT_DIR is not defined - please add build.rs to root or set env variable OUT_DIR",
//*        );
//*        out_dir.as_str().into()
//*    }
//*
//*    macro_rules! tests_with_all_kv_stores {
//*        () => {
//*            tests_with_storage!(
//*                kv_store_inmemory_tests,
//*                super::SUPPORTED_KV_STORES
//*                    .get(&crate::context::kv_store::SupportedContextKeyValueStore::InMem)
//*                    .unwrap()
//*            );
//*            tests_with_storage!(
//*                kv_store_btree_tests,
//*                super::SUPPORTED_KV_STORES
//*                    .get(&crate::context::kv_store::SupportedContextKeyValueStore::BTreeMap)
//*                    .unwrap()
//*            );
//*            tests_with_storage!(
//*                kv_store_sled_tests,
//*                super::SUPPORTED_KV_STORES
//*                    .get(
//*                        &crate::context::kv_store::SupportedContextKeyValueStore::Sled {
//*                            path: super::out_dir_path()
//*                        }
//*                    )
//*                    .unwrap()
//*            );
//*        };
//*    }
//*
//*    tests_with_all_kv_stores!();
//*}
