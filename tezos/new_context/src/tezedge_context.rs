// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    cell::RefCell,
    convert::TryInto,
    sync::{Arc, RwLock},
};
use std::{convert::TryFrom, rc::Rc};

use crypto::hash::ContextHash;
use ocaml_interop::BoxRoot;
use tezos_timing::{BlockMemoryUsage, ContextMemoryUsage};

use crate::{
    hash::ObjectHash,
    kv_store::HashId,
    persistent::DBError,
    timings::send_statistics,
    working_tree::{
        serializer::deserialize,
        storage::{BlobId, DirectoryId, NodeId, Storage},
        working_tree::{MerkleError, PostCommitData},
        working_tree_stats::MerkleStoragePerfReport,
        Commit, Object,
    },
    ContextKeyValueStore, StringDirectoryMap,
};
use crate::{working_tree::working_tree::WorkingTree, IndexApi};
use crate::{
    working_tree::working_tree::{FoldDepth, TreeWalker},
    ContextKeyOwned,
};
use crate::{
    ContextError, ContextKey, ContextValue, ProtocolContextApi, ShellContextApi, StringTreeObject,
    TreeId,
};

// Represents the patch_context function passed from the OCaml side
// It is opaque to rust, we don't care about it's actual type
// because it is not used on Rust, but we need a type to represent it.
pub struct PatchContextFunction {}

#[derive(Clone)]
pub struct TezedgeIndex {
    /// `repository` contains objects that were commited and serialized.
    /// This can be view as a map of `Hash -> object`.
    /// The `repository` contains objects from previous applied blocks, while `Self::storage`
    /// contains objects from the block being currently processed.
    pub repository: Arc<RwLock<ContextKeyValueStore>>,
    pub patch_context: Rc<Option<BoxRoot<PatchContextFunction>>>,
    /// `storage` contains all the objects from the `WorkingTree`.
    /// This is where all directories/blobs/strings are allocated.
    /// The `WorkingTree` only has access to ids which refer to data inside `storage`.
    pub storage: Rc<RefCell<Storage>>,
}

// TODO: some of the utility methods here (and in `WorkingTree`) should probably be
// standalone functions defined in a separate module that take the `index`
// as an argument.
// Also revise all errors defined, some may be obsolete now.
impl TezedgeIndex {
    pub fn new(
        repository: Arc<RwLock<ContextKeyValueStore>>,
        patch_context: Option<BoxRoot<PatchContextFunction>>,
    ) -> Self {
        let patch_context = Rc::new(patch_context);
        Self {
            patch_context,
            repository,
            storage: Default::default(),
        }
    }

    /// Fetches object from the repository associated to this `hash_id`.
    ///
    /// This returns the raw owned value (`Vec<u8>`).
    /// `Self::fetch_object` should be used to avoid allocating a `Vec<u8>`.
    pub fn fetch_object_bytes(&self, hash_id: HashId) -> Result<Option<Vec<u8>>, DBError> {
        let repo = self.repository.read()?;
        Ok(repo.get_value(hash_id)?.map(|v| v.to_vec()))
    }

    /// Fetches object from the repository and deserialize it into `Self::storage`.
    ///
    /// Returns the deserialized `Object`.
    /// If the object has not be found, this returns Ok(None).
    pub fn fetch_object(
        &self,
        hash_id: HashId,
        storage: &mut Storage,
    ) -> Result<Option<Object>, DBError> {
        let repo = self.repository.read()?;

        match repo.get_value(hash_id)? {
            None => Ok(None),
            Some(object_bytes) => Ok(Some(deserialize(object_bytes.as_ref(), storage, &*repo)?)),
        }
    }

    /// Returns the raw `ObjectHash` associated to this `hash_id`.
    ///
    /// It reads it from the repository.
    pub fn fetch_hash(&self, hash_id: HashId) -> Result<Option<ObjectHash>, DBError> {
        Ok(self
            .repository
            .read()?
            .get_hash(hash_id)?
            .map(|h| h.into_owned()))
    }

    /// Fetches object from the repository and deserialize it into `storage`.
    ///
    /// Returns an error when the object was not found.
    /// Use `Self::fetch_object` to get `None` when it has not be found.
    pub fn get_object(
        &self,
        hash_id: HashId,
        storage: &mut Storage,
    ) -> Result<Object, MerkleError> {
        match self.fetch_object(hash_id, storage)? {
            None => Err(MerkleError::ObjectNotFound { hash_id }),
            Some(object) => Ok(object),
        }
    }

    /// Fetches the commit associated to this `hash` from the repository.
    pub fn fetch_commit(
        &self,
        hash: HashId,
        storage: &mut Storage,
    ) -> Result<Option<Commit>, DBError> {
        match self.fetch_object(hash, storage)? {
            Some(Object::Commit(commit)) => Ok(Some(*commit)),
            Some(Object::Directory(_)) => Err(DBError::FoundUnexpectedStructure {
                sought: "commit".to_string(),
                found: "tree".to_string(),
            }),
            Some(Object::Blob(_)) => Err(DBError::FoundUnexpectedStructure {
                sought: "commit".to_string(),
                found: "blob".to_string(),
            }),
            None => Ok(None),
        }
    }

    /// Fetches the commit associated to this `hash_id` from the repository.
    ///
    /// Returns an error when the commit was not found.
    pub fn get_commit(
        &self,
        hash_id: HashId,
        storage: &mut Storage,
    ) -> Result<Commit, MerkleError> {
        match self.fetch_commit(hash_id, storage)? {
            None => Err(MerkleError::ObjectNotFound { hash_id }),
            Some(object) => Ok(object),
        }
    }

    /// Fetches the directory associated to this `hash` from the repository.
    ///
    /// Returns Ok(None) when there is no object associated to this `hash_id`.
    /// Returns an error when the object is not a directory.
    pub fn fetch_directory(
        &self,
        hash_id: HashId,
        storage: &mut Storage,
    ) -> Result<Option<DirectoryId>, DBError> {
        match self.fetch_object(hash_id, storage)? {
            Some(Object::Directory(dir_id)) => Ok(Some(dir_id)),
            Some(Object::Blob(_)) => Err(DBError::FoundUnexpectedStructure {
                sought: "dir".to_string(),
                found: "blob".to_string(),
            }),
            Some(Object::Commit { .. }) => Err(DBError::FoundUnexpectedStructure {
                sought: "dir".to_string(),
                found: "commit".to_string(),
            }),
            None => Ok(None),
        }
    }

    /// Fetches the commit of this `hash` from the repository.
    ///
    /// Returns an error when the commit was not found.
    pub fn get_directory(
        &self,
        hash_id: HashId,
        storage: &mut Storage,
    ) -> Result<DirectoryId, MerkleError> {
        match self.fetch_directory(hash_id, storage)? {
            None => Err(MerkleError::ObjectNotFound { hash_id }),
            Some(object) => Ok(object),
        }
    }

    /// Checks if the repository contains this `hash_id`.
    pub fn contains(&self, hash_id: HashId) -> Result<bool, DBError> {
        let db = self.repository.read()?;
        db.contains(hash_id)
    }

    /// Returns the `HashId` associated to this `context_hash`.
    ///
    /// Fetch it from the repository.
    pub fn fetch_context_hash_id(
        &self,
        context_hash: &ContextHash,
    ) -> Result<Option<HashId>, MerkleError> {
        let db = self.repository.read()?;
        Ok(db.get_context_hash(context_hash)?)
    }

    /// Convert key in array form to string form
    pub fn key_to_string(&self, key: &ContextKey) -> String {
        key.join("/")
    }

    /// Convert key in string form to array form
    pub fn string_to_key(&self, string: &str) -> ContextKeyOwned {
        string.split('/').map(str::to_string).collect()
    }

    /// Returns the object of this `node_id`.
    ///
    /// This method attemps to get the object from `Self::storage` first, and then
    /// fallbacks to the repository.
    pub fn node_object(
        &self,
        node_id: NodeId,
        storage: &mut Storage,
    ) -> Result<Object, MerkleError> {
        // get object from `Self::storage` (the working tree)

        let node = storage.get_node(node_id)?;
        if let Some(e) = node.get_object() {
            return Ok(e);
        };

        // get object by hash (from the repository)

        let hash_id = node.get_hash_id()?;
        let object = self.get_object(hash_id, storage)?;

        let node = storage.get_node(node_id)?;
        node.set_object(&object)?;

        Ok(object)
    }

    /// Get context tree under given prefix in string form (for JSON)
    /// depth - None returns full tree
    pub fn _get_context_tree_by_prefix(
        &self,
        context_hash: HashId,
        prefix: &ContextKey,
        depth: Option<usize>,
        storage: &mut Storage,
    ) -> Result<StringTreeObject, MerkleError> {
        if let Some(0) = depth {
            return Ok(StringTreeObject::Null);
        }

        let mut out = StringDirectoryMap::new();
        let commit = self.get_commit(context_hash, storage)?;

        let root_dir_id = self.get_directory(commit.root_hash, storage)?;
        let prefixed_dir_id = self.find_or_create_directory(root_dir_id, prefix, storage)?;
        let delimiter = if prefix.is_empty() { "" } else { "/" };

        let prefixed_dir = storage.dir_to_vec_unsorted(prefixed_dir_id)?;

        for (key, child_node) in prefixed_dir.iter() {
            let object = self.node_object(*child_node, storage)?;

            let key = storage.get_str(*key)?;

            // construct full path as Tree key is only one chunk of it
            let fullpath = self.key_to_string(prefix) + delimiter + key;
            let rdepth = depth.map(|d| d - 1);
            let key_str = key.to_string();

            out.insert(
                key_str,
                self.get_context_recursive(&fullpath, &object, rdepth, storage)?,
            );
        }

        //stat_updater.update_execution_stats(&mut self.stats);
        Ok(StringTreeObject::Directory(out))
    }

    /// Go recursively down the tree from Object, build string tree and return it
    /// (or return hex value if Blob)
    fn get_context_recursive(
        &self,
        path: &str,
        object: &Object,
        depth: Option<usize>,
        storage: &mut Storage,
    ) -> Result<StringTreeObject, MerkleError> {
        if let Some(0) = depth {
            return Ok(StringTreeObject::Null);
        }

        match object {
            Object::Blob(blob_id) => {
                let blob = storage.get_blob(*blob_id)?;
                Ok(StringTreeObject::Blob(hex::encode(blob)))
            }
            Object::Directory(dir_id) => {
                let mut new_tree = StringDirectoryMap::new();

                let dir = storage.dir_to_vec_unsorted(*dir_id)?;

                for (key, child_node) in dir.iter() {
                    let key = storage.get_str(*key)?;
                    let fullpath = path.to_owned() + "/" + key;
                    let key_str = key.to_string();

                    let object = self.node_object(*child_node, storage)?;
                    let rdepth = depth.map(|d| d - 1);

                    new_tree.insert(
                        key_str,
                        self.get_context_recursive(&fullpath, &object, rdepth, storage)?,
                    );
                }
                Ok(StringTreeObject::Directory(new_tree))
            }
            Object::Commit(_) => Err(MerkleError::FoundUnexpectedStructure {
                sought: "Directory/Blob".to_string(),
                found: "Commit".to_string(),
            }),
        }
    }

    /// Traverses `root` and returns the directory at `path`.
    ///
    /// Fetches objects from the repository if necessary,
    /// Return an empty directory if no directory under this path exists or if a blob
    /// (= value) is encountered along the way.
    ///
    pub fn find_raw_tree(
        &self,
        root: Tree,
        key: &[&str],
        storage: &mut Storage,
    ) -> Result<Tree, MerkleError> {
        let first = match key.first() {
            Some(first) => *first,
            None => {
                // terminate recursion if end of path was reached
                return Ok(root);
            }
        };

        // first get node at key
        let child_node_id = match storage.tree_find_node(root, first) {
            Some(node_id) => node_id,
            None => {
                return Ok(Tree::empty());
            }
        };

        // get entry (from working tree)
        let child_node = storage.get_node(child_node_id)?;
        match child_node.get_entry() {
            Some(Entry::Tree(tree)) => {
                return self.find_raw_tree(tree, &key[1..], storage);
            }
            Some(Entry::Blob(_)) => return Ok(Tree::empty()),
            Some(Entry::Commit { .. }) => {
                return Err(MerkleError::FoundUnexpectedStructure {
                    sought: "Tree/Blob".to_string(),
                    found: "commit".to_string(),
                })
            }
            // If `Node::get_entry` returns `None`, it means that the entry
            // is not deserialized into the working tree.
            // To get the entry, we must fetch it from the repository.
            // See below.
            None => {}
        }

        // get entry by hash (from DB)
        let hash = child_node.get_hash_id()?;
        let entry = self.get_entry(hash, storage)?;

        let child_node = storage.get_node(child_node_id)?;
        child_node.set_entry(&entry)?;

        match entry {
            Entry::Tree(tree) => self.find_raw_tree(tree, &key[1..], storage),
            Entry::Blob(_) => Ok(Tree::empty()),
            Entry::Commit { .. } => Err(MerkleError::FoundUnexpectedStructure {
                sought: "Tree/Blob".to_string(),
                found: "commit".to_string(),
            }),
        }
    }

    /// Get value from historical context identified by commit hash.
    pub fn get_history(
        &self,
        commit_hash: HashId,
        key: &ContextKey,
    ) -> Result<ContextValue, MerkleError> {
        let mut storage = (&*self.storage).borrow_mut();

        let commit = self.get_commit(commit_hash, &mut storage)?;
        let dir_id = self.get_directory(commit.root_hash, &mut storage)?;

        let blob_id = self.get_from_tree(tree, key, &mut storage)?;
        let blob = storage.get_blob(blob_id)?;

        Ok(blob.to_vec())
    }

    fn get_from_tree(
        &self,
        root: Tree,
        key: &ContextKey,
        storage: &mut Storage,
    ) -> Result<BlobStorageId, MerkleError> {
        let (file, path) = key.split_last().ok_or(MerkleError::KeyEmpty)?;

        let node = self.find_raw_tree(root, &path, storage)?;

        // get file node from tree
        let node_id =
            storage
                .tree_find_node(node, *file)
                .ok_or_else(|| MerkleError::ValueNotFound {
                    key: self.key_to_string(key),
                })?;

        // get blob
        match self.node_entry(node_id, storage)? {
            Entry::Blob(blob) => Ok(blob),
            _ => Err(MerkleError::ValueIsNotABlob {
                key: self.key_to_string(key),
            }),
        }
    }

    /// Construct Vec of all context key-values under given prefix
    pub fn get_context_key_values_by_prefix(
        &self,
        context_hash: HashId,
        prefix: &ContextKey,
    ) -> Result<Option<Vec<(ContextKeyOwned, ContextValue)>>, MerkleError> {
        let mut storage = (&*self.storage).borrow_mut();

        let commit = self.get_commit(context_hash, &mut storage)?;
        let root_dir_id = self.get_directory(commit.root_hash, &mut storage)?;
        self.get_context_key_values_by_prefix_impl(root_dir_id, prefix, &mut storage)
    }

    /// Implementation of `Self::get_context_key_values_by_prefix`
    fn get_context_key_values_by_prefix_impl(
        &self,
        root_dir: DirectoryId,
        prefix: &ContextKey,
        storage: &mut Storage,
    ) -> Result<Option<Vec<(ContextKeyOwned, ContextValue)>>, MerkleError> {
        let prefixed_dir_id = self.find_or_create_directory(root_dir, prefix, storage)?;
        let mut keyvalues: Vec<(ContextKeyOwned, ContextValue)> = Vec::new();
        let delimiter = if prefix.is_empty() { "" } else { "/" };

        let prefixed_dir = storage.dir_to_vec_unsorted(prefixed_dir_id)?;

        for (key, child_node) in prefixed_dir.iter() {
            let object = self.node_object(*child_node, storage)?;

            let key = storage.get_str(*key)?;
            // construct full path as Tree key is only one chunk of it
            let fullpath = self.key_to_string(prefix) + delimiter + key;

            self.collect_key_values_from_tree_recursively(
                &fullpath,
                &object,
                &mut keyvalues,
                storage,
            )?;
        }

        if keyvalues.is_empty() {
            Ok(None)
        } else {
            Ok(Some(keyvalues))
        }
    }

    /// Traverses `object` and all its children, and collect all the value (= blobs)
    /// and their full paths into `entries`.
    ///
    // TODO: can we get rid of the recursion?
    fn collect_key_values_from_tree_recursively(
        &self,
        path: &str,
        object: &Object,
        entries: &mut Vec<(ContextKeyOwned, ContextValue)>,
        storage: &mut Storage,
    ) -> Result<(), MerkleError> {
        match object {
            Object::Blob(blob_id) => {
                // push key-value pair
                let blob = storage.get_blob(*blob_id)?;
                entries.push((self.string_to_key(path), blob.to_vec()));
                Ok(())
            }
            Object::Directory(dir_id) => {
                let dir = storage.dir_to_vec_unsorted(*dir_id)?;

                dir.iter()
                    .map(|(key, child_node_id)| {
                        let key = storage.get_str(*key)?;
                        let fullpath = path.to_owned() + "/" + key;

                        match self.node_object(*child_node_id, storage) {
                            Err(_) => Ok(()),
                            Ok(object) => self.collect_key_values_from_tree_recursively(
                                &fullpath, &object, entries, storage,
                            ),
                        }
                    })
                    .find_map(|res| match res {
                        Ok(_) => None,
                        Err(err) => Some(Err(err)),
                    })
                    .unwrap_or(Ok(()))
            }
            Object::Commit(commit) => match self.get_object(commit.root_hash, storage) {
                Err(err) => Err(err),
                Ok(object) => {
                    self.collect_key_values_from_tree_recursively(path, &object, entries, storage)
                }
            },
        }
    }
}

impl IndexApi<TezedgeContext> for TezedgeIndex {
    /// Checks if `context_hash` exists in the repository.
    fn exists(&self, context_hash: &ContextHash) -> Result<bool, ContextError> {
        let hash_id = {
            let repository = self.repository.read()?;

            match repository.get_context_hash(context_hash)? {
                Some(hash_id) => hash_id,
                None => return Ok(false),
            }
        };

        let mut storage = self.storage.borrow_mut();

        if let Some(Object::Commit(_)) = self.fetch_object(hash_id, &mut storage)? {
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn checkout(&self, context_hash: &ContextHash) -> Result<Option<TezedgeContext>, ContextError> {
        let hash_id = {
            let repository = self.repository.read()?;

            match repository.get_context_hash(context_hash)? {
                Some(hash_id) => hash_id,
                None => return Ok(None),
            }
        };

        let mut storage = self.storage.borrow_mut();
        storage.clear();

        let commit = match self.fetch_commit(hash_id, &mut storage)? {
            Some(commit) => commit,
            None => return Ok(None),
        };

        let dir_id = match self.fetch_directory(commit.root_hash, &mut storage)? {
            Some(dir_id) => dir_id,
            None => return Ok(None),
        };

        let tree = WorkingTree::new_with_directory(self.clone(), dir_id);

        Ok(Some(TezedgeContext::new(
            self.clone(),
            Some(hash_id),
            Some(Rc::new(tree)),
        )))
    }

    fn block_applied(&self, referenced_older_entries: Vec<HashId>) -> Result<(), ContextError> {
        Ok(self
            .repository
            .write()?
            .block_applied(referenced_older_entries)?)
    }

    fn cycle_started(&mut self) -> Result<(), ContextError> {
        Ok(self.repository.write()?.new_cycle_started()?)
    }

    fn get_key_from_history(
        &self,
        context_hash: &ContextHash,
        key: &ContextKey,
    ) -> Result<Option<ContextValue>, ContextError> {
        let hash_id = {
            let repository = self.repository.read()?;

            match repository.get_context_hash(context_hash)? {
                Some(hash_id) => hash_id,
                None => {
                    return Err(ContextError::UnknownContextHashError {
                        context_hash: context_hash.to_base58_check(),
                    })
                }
            }
        };

        match self.get_history(hash_id, key) {
            Err(MerkleError::ValueNotFound { key: _ }) => Ok(None),
            Err(MerkleError::ObjectNotFound { hash_id: _ }) => Ok(None),
            Err(err) => Err(ContextError::MerkleStorageError { error: err }),
            Ok(val) => Ok(Some(val)),
        }
    }

    fn get_key_values_by_prefix(
        &self,
        context_hash: &ContextHash,
        prefix: &ContextKey,
    ) -> Result<Option<Vec<(ContextKeyOwned, ContextValue)>>, ContextError> {
        let hash_id = {
            let repository = self.repository.read()?;
            match repository.get_context_hash(context_hash)? {
                Some(hash_id) => hash_id,
                None => {
                    return Err(ContextError::UnknownContextHashError {
                        context_hash: context_hash.to_base58_check(),
                    })
                }
            }
        };

        self.get_context_key_values_by_prefix(hash_id, prefix)
            .map_err(Into::into)
    }

    fn get_context_tree_by_prefix(
        &self,
        context_hash: &ContextHash,
        prefix: &ContextKey,
        depth: Option<usize>,
    ) -> Result<StringTreeObject, ContextError> {
        let hash_id = {
            let repository = self.repository.read()?;
            match repository.get_context_hash(context_hash)? {
                Some(hash_id) => hash_id,
                None => {
                    return Err(ContextError::UnknownContextHashError {
                        context_hash: context_hash.to_base58_check(),
                    })
                }
            }
        };

        let mut storage = self.storage.borrow_mut();

        self._get_context_tree_by_prefix(hash_id, prefix, depth, &mut storage)
            .map_err(ContextError::from)
    }
}

// context implementation using merkle-tree-like storage
#[derive(Clone)]
pub struct TezedgeContext {
    pub index: TezedgeIndex,
    pub parent_commit_hash: Option<HashId>,
    pub tree_id: TreeId,
    tree_id_generator: Rc<RefCell<TreeIdGenerator>>,
    pub tree: Rc<WorkingTree>,
}

impl ProtocolContextApi for TezedgeContext {
    fn add(&self, key: &ContextKey, value: &[u8]) -> Result<Self, ContextError> {
        let tree = self.tree.add(key, value)?;

        Ok(self.with_tree(tree))
    }

    fn delete(&self, key_prefix_to_delete: &ContextKey) -> Result<Self, ContextError> {
        let tree = self.tree.delete(key_prefix_to_delete)?;

        Ok(self.with_tree(tree))
    }

    fn find(&self, key: &ContextKey) -> Result<Option<ContextValue>, ContextError> {
        Ok(self.tree.find(key)?)
    }

    fn mem(&self, key: &ContextKey) -> Result<bool, ContextError> {
        Ok(self.tree.mem(key)?)
    }

    fn mem_tree(&self, key: &ContextKey) -> bool {
        self.tree.mem_tree(key)
    }

    fn find_tree(&self, key: &ContextKey) -> Result<Option<WorkingTree>, ContextError> {
        self.tree.find_tree(key).map_err(Into::into)
    }

    fn add_tree(&self, key: &ContextKey, tree: &WorkingTree) -> Result<Self, ContextError> {
        Ok(self.with_tree(self.tree.add_tree(key, tree)?))
    }

    fn empty(&self) -> Self {
        self.with_tree(self.tree.empty())
    }

    fn list(
        &self,
        offset: Option<usize>,
        length: Option<usize>,
        key: &ContextKey,
    ) -> Result<Vec<(String, WorkingTree)>, ContextError> {
        self.tree.list(offset, length, key).map_err(Into::into)
    }

    fn fold_iter(
        &self,
        depth: Option<FoldDepth>,
        key: &ContextKey,
    ) -> Result<TreeWalker, ContextError> {
        Ok(self.tree.fold_iter(depth, key)?)
    }

    fn get_merkle_root(&self) -> Result<ObjectHash, ContextError> {
        self.tree.hash().map_err(Into::into)
    }
}

impl ShellContextApi for TezedgeContext {
    fn commit(
        &self,
        author: String,
        message: String,
        date: i64,
    ) -> Result<ContextHash, ContextError> {
        // Entries to be inserted are obtained from the commit call and written here
        let date: u64 = date.try_into()?;
        let mut repository = self.index.repository.write()?;

        let PostCommitData {
            commit_hash_id,
            batch,
            reused,
            serialize_stats,
        } = self.tree.prepare_commit(
            date,
            author,
            message,
            self.parent_commit_hash,
            &mut *repository,
            true,
        )?;

        // FIXME: only write entries if there are any, empty commits should not produce anything
        repository.write_batch(batch)?;
        repository.put_context_hash(commit_hash_id)?;
        repository.block_applied(reused)?;

        let commit_hash = self.get_commit_hash(commit_hash_id, &*repository)?;
        repository.clear_entries()?;

        std::mem::drop(repository);
        send_statistics(BlockMemoryUsage {
            context: Box::new(self.get_memory_usage()?),
            serialize: serialize_stats,
        });

        Ok(commit_hash)
    }

    fn hash(
        &self,
        author: String,
        message: String,
        date: i64,
    ) -> Result<ContextHash, ContextError> {
        let date: u64 = date.try_into()?;
        let mut repository = self.index.repository.write()?;

        let PostCommitData { commit_hash_id, .. } = self.tree.prepare_commit(
            date,
            author,
            message,
            self.parent_commit_hash,
            &mut *repository,
            false,
        )?;

        let commit_hash = self.get_commit_hash(commit_hash_id, &*repository)?;
        repository.clear_entries()?;
        Ok(commit_hash)
    }

    fn get_last_commit_hash(&self) -> Result<Option<Vec<u8>>, ContextError> {
        let repository = self.index.repository.read()?;

        let value = match self.parent_commit_hash {
            Some(hash_id) => repository.get_value(hash_id)?,
            None => return Ok(None),
        };

        Ok(value.map(|v| v.to_vec()))
    }

    fn get_merkle_stats(&self) -> Result<MerkleStoragePerfReport, ContextError> {
        Ok(MerkleStoragePerfReport::default())
    }

    fn get_memory_usage(&self) -> Result<ContextMemoryUsage, ContextError> {
        let repository = self.index.repository.read()?;
        let storage = (&*self.index.storage).borrow();

        let usage = ContextMemoryUsage {
            repo: repository.memory_usage(),
            storage: storage.memory_usage(),
        };

        Ok(usage)
    }
}

/// Generator of Tree IDs which are used to simulate pointers when they are not available.
///
/// During a regular use of the context API, contexts that are still in use are kept
/// alive by pointers to them. This is not available when for example, running the context
/// actions replayer tool. To solve that, we generate a tree id for each versionf of the
/// working tree that is produced while applying a block, so that actions can be associated
/// to the tree to which they are applied.
pub struct TreeIdGenerator(TreeId);

impl TreeIdGenerator {
    fn new() -> Self {
        Self(0)
    }

    fn next(&mut self) -> TreeId {
        self.0 += 1;
        self.0
    }
}

impl TezedgeContext {
    // NOTE: only used to start from scratch, otherwise checkout should be used
    pub fn new(
        index: TezedgeIndex,
        parent_commit_hash: Option<HashId>,
        tree: Option<Rc<WorkingTree>>,
    ) -> Self {
        let tree = if let Some(tree) = tree {
            tree
        } else {
            Rc::new(WorkingTree::new(index.clone()))
        };
        let tree_id_generator = Rc::new(RefCell::new(TreeIdGenerator::new()));
        let tree_id = tree_id_generator.borrow_mut().next();
        Self {
            index,
            parent_commit_hash,
            tree_id,
            tree_id_generator,
            tree,
        }
    }

    /// Produce a new copy of the context, replacing the tree (and if different, with a new tree id)
    pub fn with_tree(&self, tree: WorkingTree) -> Self {
        // TODO: only generate a new id if tree changes? Either that
        // or generate a new one every time for Irmin even if the tree doesn't change
        let tree_id = self.tree_id_generator.borrow_mut().next();
        let tree = Rc::new(tree);
        Self {
            tree,
            tree_id,
            tree_id_generator: Rc::clone(&self.tree_id_generator),
            index: self.index.clone(),
            ..*self
        }
    }

    fn get_commit_hash(
        &self,
        commit_hash_id: HashId,
        repo: &ContextKeyValueStore,
    ) -> Result<ContextHash, ContextError> {
        let commit_hash = match repo.get_hash(commit_hash_id)? {
            Some(hash) => hash,
            None => {
                return Err(MerkleError::ObjectNotFound {
                    hash_id: commit_hash_id,
                }
                .into())
            }
        };
        let commit_hash = ContextHash::try_from(&commit_hash[..])?;
        Ok(commit_hash)
    }
}

#[cfg(test)]
mod tests {
    use tezos_api::ffi::{ContextKvStoreConfiguration, TezosContextTezEdgeStorageConfiguration};

    use super::*;
    use crate::initializer::initialize_tezedge_context;

    #[test]
    fn init_context() {
        let context = initialize_tezedge_context(&TezosContextTezEdgeStorageConfiguration {
            backend: ContextKvStoreConfiguration::InMem,
            ipc_socket_path: None,
        })
        .unwrap();

        // Context is immutable so on any modification, the methods return the new tree
        let context = context.add(&["a", "b", "c"], &[1, 2, 3]).unwrap();
        let context = context.add(&["m", "n", "o"], &[4, 5, 6]).unwrap();
        assert_eq!(context.find(&["a", "b", "c"]).unwrap().unwrap(), &[1, 2, 3]);

        let context2 = context.delete(&["m", "n", "o"]).unwrap();
        assert!(context.mem(&["m", "n", "o"]).unwrap());
        assert!(context2.mem(&["m", "n", "o"]).unwrap() == false);

        assert!(context.mem_tree(&["a"]));

        let tree_a = context.find_tree(&["a"]).unwrap().unwrap();
        let context = context.add_tree(&["z"], &tree_a).unwrap();

        assert_eq!(
            context.find(&["a", "b", "c"]).unwrap().unwrap(),
            context.find(&["z", "b", "c"]).unwrap().unwrap(),
        );

        let context = context.add(&["a", "b1", "c"], &[10, 20, 30]).unwrap();
        let list = context.list(None, None, &["a"]).unwrap();

        assert_eq!(&*list[0].0, "b");
        assert_eq!(&*list[1].0, "b1");

        assert_eq!(
            context.get_merkle_root().unwrap(),
            [
                1, 217, 94, 141, 166, 51, 65, 3, 104, 220, 208, 35, 122, 106, 131, 147, 183, 133,
                81, 239, 195, 111, 25, 29, 88, 1, 46, 251, 25, 205, 202, 229
            ]
        );
    }
}
