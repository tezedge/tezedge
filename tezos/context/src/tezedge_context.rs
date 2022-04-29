// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! This module implements the Tezos context API.

use std::rc::Rc;
use std::{cell::RefCell, convert::TryInto, sync::Arc};

use crypto::hash::ContextHash;
use ocaml_interop::BoxRoot;
use parking_lot::RwLock;
use tezos_context_api::StringDirectoryMap;
use tezos_timing::{BlockMemoryUsage, ContextMemoryUsage};

use crate::working_tree::working_tree::FoldOrder;
use crate::{
    hash::ObjectHash,
    kv_store::HashId,
    persistent::{get_commit_hash, DBError},
    timings::send_statistics,
    working_tree::{
        storage::{BlobId, DirEntryId, DirectoryId, Storage},
        string_interner::StringInterner,
        working_tree::{MerkleError, PostCommitData},
        Commit, Object, ObjectReference,
    },
    ContextKeyValueStore,
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

struct TezedgeIndexWithDeallocation<'a> {
    index: &'a TezedgeIndex,
}

impl std::ops::Deref for TezedgeIndexWithDeallocation<'_> {
    type Target = TezedgeIndex;

    fn deref(&self) -> &Self::Target {
        self.index
    }
}

impl Drop for TezedgeIndexWithDeallocation<'_> {
    fn drop(&mut self) {
        let mut storage = self.storage.borrow_mut();
        storage.deallocate();
    }
}

struct TezedgeContextWithDeallocation<'a> {
    ctx: &'a TezedgeContext,
}

impl std::ops::Deref for TezedgeContextWithDeallocation<'_> {
    type Target = TezedgeContext;

    fn deref(&self) -> &Self::Target {
        self.ctx
    }
}

impl Drop for TezedgeContextWithDeallocation<'_> {
    fn drop(&mut self) {
        let mut storage = self.index.storage.borrow_mut();
        storage.deallocate();
    }
}

/// The index is how we interact with the actual storage used to store the
/// context data. All reading and writing to the storage is done through the index.
#[derive(Clone)]
pub struct TezedgeIndex {
    /// `repository` contains objects that were committed and serialized.
    /// This can be view as a map of `Hash -> object`.
    /// The `repository` contains objects from previous applied blocks, while `Self::storage`
    /// contains objects from the block being currently processed.
    pub repository: Arc<RwLock<ContextKeyValueStore>>,
    pub patch_context: Rc<Option<BoxRoot<PatchContextFunction>>>,
    /// `storage` contains all the objects from the `WorkingTree`.
    /// This is where all directories/blobs/strings are allocated.
    /// The `WorkingTree` only has access to ids which refer to data inside `storage`.
    pub storage: Rc<RefCell<Storage>>,
    /// Container of all the strings.
    /// This is never deallocated.
    /// The working tree and repository have `StringId` which refers to
    /// a data inside `StringInterner`.
    pub string_interner: Rc<RefCell<Option<StringInterner>>>,
}

use std::cell::RefMut;

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
            string_interner: Rc::new(RefCell::new(None)),
        }
    }

    pub fn with_storage(
        repository: Arc<RwLock<ContextKeyValueStore>>,
        storage: Rc<RefCell<Storage>>,
        string_interner: Rc<RefCell<Option<StringInterner>>>,
    ) -> Self {
        Self {
            patch_context: Default::default(),
            repository,
            storage,
            string_interner,
        }
    }

    pub fn get_string_interner(&self) -> Result<RefMut<StringInterner>, DBError> {
        // When the context is reloaded/restarted, the existings strings (found the the db file)
        // are in the repository.
        // We want `TezedgeIndex` to have its string interner updated with the one
        // from the repository.

        let mut strings: RefMut<Option<StringInterner>> = self.string_interner.borrow_mut();

        if strings.is_some() {
            // The strings are already in `TezedgeIndex`
            return Ok(RefMut::map(strings, |s| s.as_mut().unwrap())); // Do no fail
        }
        // Strings are not in `TezedgeIndex`, we need to request them to
        // the repository
        let mut repo = self.repository.write();
        let string_interner = repo.take_strings_on_reload();

        // `unwrap_or_default` because only the persistent backend returns
        // the strings on `take_string_on_reload`, with other backends,
        // we create a default `StringInterner`.
        strings.replace(string_interner.unwrap_or_default());

        // Do not fail, we just replace the `Option`
        Ok(RefMut::map(strings, |s| s.as_mut().unwrap()))
    }

    pub fn fetch_commit_from_context_hash(
        &self,
        context_hash: &ContextHash,
    ) -> Result<Option<Commit>, ContextError> {
        let object_ref = {
            let repository = self.repository.read();

            match repository.get_context_hash(context_hash)? {
                Some(hash_id) => hash_id,
                None => return Ok(None),
            }
        };

        let mut storage = self.storage.borrow_mut();
        let mut strings = self.get_string_interner()?;

        match self.fetch_commit(object_ref, &mut storage, &mut strings)? {
            Some(commit) => Ok(Some(commit)),
            None => Ok(None),
        }
    }

    fn with_deallocation(&self) -> TezedgeIndexWithDeallocation {
        TezedgeIndexWithDeallocation { index: self }
    }

    /// Fetches object from the repository and deserialize it into `Self::storage`.
    ///
    /// Returns the deserialized `Object`.
    /// If the object has not be found, this returns Ok(None).
    pub fn fetch_object(
        &self,
        object_ref: ObjectReference,
        storage: &mut Storage,
        strings: &mut StringInterner,
    ) -> Result<Option<Object>, DBError> {
        let repository = self.repository.read();

        repository
            .get_object(object_ref, storage, strings)
            .map(Some)
    }

    /// Returns the raw `ObjectHash` associated to this `hash_id`.
    ///
    /// It reads it from the repository.
    pub fn fetch_hash(&self, object_ref: ObjectReference) -> Result<ObjectHash, DBError> {
        Ok(self.repository.read().get_hash(object_ref)?.into_owned())
    }

    /// Fetches object from the repository and deserialize it into `storage`.
    ///
    /// Returns an error when the object was not found.
    /// Use `Self::fetch_object` to get `None` when it has not be found.
    pub fn get_object(
        &self,
        object_ref: ObjectReference,
        storage: &mut Storage,
        strings: &mut StringInterner,
    ) -> Result<Object, MerkleError> {
        match self.fetch_object(object_ref, storage, strings)? {
            Some(object) => Ok(object),
            None => Err(MerkleError::ObjectNotFound { object_ref }),
        }
    }

    /// Fetches the commit associated to this `hash` from the repository.
    pub fn fetch_commit(
        &self,
        object_ref: ObjectReference,
        storage: &mut Storage,
        strings: &mut StringInterner,
    ) -> Result<Option<Commit>, DBError> {
        match self.fetch_object(object_ref, storage, strings)? {
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
        object_ref: ObjectReference,
        storage: &mut Storage,
        strings: &mut StringInterner,
    ) -> Result<Commit, MerkleError> {
        match self.fetch_commit(object_ref, storage, strings)? {
            None => Err(MerkleError::ObjectNotFound { object_ref }),
            Some(object) => Ok(object),
        }
    }

    /// Fetches the directory associated to this `hash` from the repository.
    ///
    /// Returns Ok(None) when there is no object associated to this `hash_id`.
    /// Returns an error when the object is not a directory.
    pub fn fetch_directory(
        &self,
        object_ref: ObjectReference,
        storage: &mut Storage,
        strings: &mut StringInterner,
    ) -> Result<Option<DirectoryId>, DBError> {
        match self.fetch_object(object_ref, storage, strings)? {
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
        object_ref: ObjectReference,
        storage: &mut Storage,
        strings: &mut StringInterner,
    ) -> Result<DirectoryId, MerkleError> {
        match self.fetch_directory(object_ref, storage, strings)? {
            None => Err(MerkleError::ObjectNotFound { object_ref }),
            Some(object) => Ok(object),
        }
    }

    /// Checks if the repository contains this `hash_id`.
    pub fn contains(&self, hash_id: HashId) -> Result<bool, DBError> {
        let repository = self.repository.read();
        repository.contains(hash_id)
    }

    /// Returns the `HashId` associated to this `context_hash`.
    ///
    /// Fetch it from the repository.
    pub fn fetch_context_hash_id(
        &self,
        context_hash: &ContextHash,
    ) -> Result<Option<ObjectReference>, MerkleError> {
        let repository = self.repository.read();
        Ok(repository.get_context_hash(context_hash)?)
    }

    /// Convert key in array form to string form
    pub fn key_to_string(&self, key: &ContextKey) -> String {
        key.join("/")
    }

    /// Convert key in string form to array form
    pub fn string_to_key(&self, string: &str) -> ContextKeyOwned {
        string.split('/').map(str::to_string).collect()
    }

    /// Returns the object of this `dir_entry_id`.
    ///
    /// This method attemps to get the object from `Self::storage` first, and then
    /// fallbacks to the repository.
    pub fn dir_entry_object(
        &self,
        dir_entry_id: DirEntryId,
        storage: &mut Storage,
        strings: &mut StringInterner,
    ) -> Result<Object, MerkleError> {
        // get object from `Self::storage` (the working tree)

        let dir_entry = storage.get_dir_entry(dir_entry_id)?;
        if let Some(e) = dir_entry.get_object() {
            return Ok(e);
        };

        // get object by hash (from the repository)

        let object_ref = dir_entry.get_reference();
        let object = self.get_object(object_ref, storage, strings)?;

        let dir_entry = storage.get_dir_entry(dir_entry_id)?;
        dir_entry.set_object(&object)?;

        Ok(object)
    }

    /// Get context tree under given prefix in string form (for JSON)
    /// depth - None returns full tree
    pub fn _get_context_tree_by_prefix(
        &self,
        object_ref: ObjectReference,
        prefix: &ContextKey,
        depth: Option<usize>,
        storage: &mut Storage,
        strings: &mut StringInterner,
        repository: &ContextKeyValueStore,
    ) -> Result<StringTreeObject, MerkleError> {
        if let Some(0) = depth {
            return Ok(StringTreeObject::Null);
        }

        let mut out = StringDirectoryMap::new();
        let commit = self.get_commit(object_ref, storage, strings)?;

        let root_dir_id = self.get_directory(commit.root_ref, storage, strings)?;
        let prefixed_dir_id =
            self.find_or_create_directory(root_dir_id, prefix, storage, strings)?;
        let delimiter = if prefix.is_empty() { "" } else { "/" };

        let prefixed_dir = storage.dir_to_vec_unsorted(prefixed_dir_id, strings, repository)?;

        for (key, child_dir_entry) in prefixed_dir.iter() {
            let object = self.dir_entry_object(*child_dir_entry, storage, strings)?;

            let key = strings.get_str(*key)?;

            // construct full path as Tree key is only one chunk of it
            let fullpath = self.key_to_string(prefix) + delimiter + &key;
            let rdepth = depth.map(|d| d - 1);
            let key_str = key.to_string();

            out.insert(
                key_str,
                self.get_context_recursive(
                    &fullpath, &object, rdepth, storage, strings, repository,
                )?,
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
        strings: &mut StringInterner,
        repository: &ContextKeyValueStore,
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

                let dir = storage.dir_to_vec_unsorted(*dir_id, strings, repository)?;

                for (key, child_dir_entry) in dir.iter() {
                    let key = strings.get_str(*key)?;
                    let fullpath = path.to_owned() + "/" + &key;
                    let key_str = key.to_string();

                    let object = self.dir_entry_object(*child_dir_entry, storage, strings)?;
                    let rdepth = depth.map(|d| d - 1);

                    new_tree.insert(
                        key_str,
                        self.get_context_recursive(
                            &fullpath, &object, rdepth, storage, strings, repository,
                        )?,
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
    /// Use `Self::find_dir_entry` to get the `DirEntryId` at that path.
    pub fn find_or_create_directory(
        &self,
        root: DirectoryId,
        path: &ContextKey,
        storage: &mut Storage,
        strings: &mut StringInterner,
    ) -> Result<DirectoryId, MerkleError> {
        if path.is_empty() {
            return Ok(root);
        }

        let dir_entry_id = match self.find_dir_entry(root, path, storage, strings)? {
            Some(dir_entry_id) => dir_entry_id,
            None => return Ok(DirectoryId::empty()),
        };

        match self.dir_entry_object(dir_entry_id, storage, strings)? {
            Object::Directory(dir_id) => Ok(dir_id),
            Object::Blob(_) => Ok(DirectoryId::empty()),
            Object::Commit(_) => Err(MerkleError::FoundUnexpectedStructure {
                sought: "Directory/Blob".to_string(),
                found: "Commit".to_string(),
            }),
        }
    }

    /// Traverses `root` and returns the dir_entry at `path`.
    ///
    /// Fetches objects from the repository if necessary.
    /// Returns `None` if the path doesn't exist or if a blob is encountered.
    pub fn find_dir_entry(
        &self,
        mut root: DirectoryId,
        path: &ContextKey,
        storage: &mut Storage,
        strings: &mut StringInterner,
    ) -> Result<Option<DirEntryId>, MerkleError> {
        if path.is_empty() {
            return Err(MerkleError::KeyEmpty);
        }

        let last_key_index = path.len() - 1;
        let repository = self.repository.read();

        for (index, key) in path.iter().enumerate() {
            let child_dir_entry_id =
                match storage.dir_find_dir_entry(root, key, strings, &*repository)? {
                    Some(dir_entry_id) => dir_entry_id,
                    None => return Ok(None), // Path doesn't exist
                };

            if index == last_key_index {
                // We reached the last key in the path, return the `DirEntryId`.
                return Ok(Some(child_dir_entry_id));
            }

            match self.dir_entry_object(child_dir_entry_id, storage, strings)? {
                Object::Directory(dir_id) => {
                    // Go to next key
                    root = dir_id;
                }
                Object::Blob(_) => return Ok(None),
                Object::Commit(_) => {
                    return Err(MerkleError::FoundUnexpectedStructure {
                        sought: "Directory/Blob".to_string(),
                        found: "Commit".to_string(),
                    })
                }
            };
        }

        unreachable!()
    }

    /// Get value from historical context identified by commit hash.
    pub fn get_history(
        &self,
        object_ref: ObjectReference,
        key: &ContextKey,
    ) -> Result<ContextValue, MerkleError> {
        let mut storage = self.storage.borrow_mut();
        let mut strings = self.get_string_interner()?;

        let commit = self.get_commit(object_ref, &mut storage, &mut strings)?;

        let dir_id = self.get_directory(commit.root_ref, &mut storage, &mut strings)?;

        let blob_id = self.try_find_blob(dir_id, key, &mut storage, &mut strings)?;
        let blob = storage.get_blob(blob_id)?;

        Ok(blob.to_vec())
    }

    /// Traverses `root` and returns the value (= blob) at `path`.
    ///
    /// Fetches objects from repository if necessary.
    /// Returns an error if the path doesn't exist or is not a blob.
    ///
    /// Use `Self::find_dir_entry` to get the `DirEntryId` at that path.
    pub fn try_find_blob(
        &self,
        root: DirectoryId,
        key: &ContextKey,
        storage: &mut Storage,
        strings: &mut StringInterner,
    ) -> Result<BlobId, MerkleError> {
        let dir_entry_id = match self.find_dir_entry(root, key, storage, strings)? {
            Some(dir_entry_id) => dir_entry_id,
            None => {
                return Err(MerkleError::ValueNotFound {
                    key: self.key_to_string(key),
                });
            }
        };

        // get blob
        match self.dir_entry_object(dir_entry_id, storage, strings)? {
            Object::Blob(blob) => Ok(blob),
            Object::Directory(_) => Err(MerkleError::ValueIsNotABlob {
                key: self.key_to_string(key),
            }),
            Object::Commit(_) => Err(MerkleError::ValueIsNotABlob {
                key: self.key_to_string(key),
            }),
        }
    }

    /// Construct Vec of all context key-values under given prefix
    pub fn get_context_key_values_by_prefix(
        &self,
        object_ref: ObjectReference,
        prefix: &ContextKey,
    ) -> Result<Option<Vec<(ContextKeyOwned, ContextValue)>>, MerkleError> {
        let mut storage = self.storage.borrow_mut();
        let repository = self.repository.read();
        let mut strings = self.get_string_interner()?;

        let commit = self.get_commit(object_ref, &mut storage, &mut strings)?;

        let root_dir_id = self.get_directory(commit.root_ref, &mut storage, &mut strings)?;
        self.get_context_key_values_by_prefix_impl(
            root_dir_id,
            prefix,
            &mut storage,
            &mut strings,
            &*repository,
        )
    }

    /// Implementation of `Self::get_context_key_values_by_prefix`
    fn get_context_key_values_by_prefix_impl(
        &self,
        root_dir: DirectoryId,
        prefix: &ContextKey,
        storage: &mut Storage,
        strings: &mut StringInterner,
        repository: &ContextKeyValueStore,
    ) -> Result<Option<Vec<(ContextKeyOwned, ContextValue)>>, MerkleError> {
        let prefixed_dir_id = self.find_or_create_directory(root_dir, prefix, storage, strings)?;
        let mut keyvalues: Vec<(ContextKeyOwned, ContextValue)> = Vec::new();
        let delimiter = if prefix.is_empty() { "" } else { "/" };

        let prefixed_dir = storage.dir_to_vec_unsorted(prefixed_dir_id, strings, repository)?;

        for (key, child_dir_entry) in prefixed_dir.iter() {
            let object = self.dir_entry_object(*child_dir_entry, storage, strings)?;

            let key = strings.get_str(*key)?;
            // construct full path as Tree key is only one chunk of it
            let fullpath = self.key_to_string(prefix) + delimiter + &key;

            self.collect_key_values_from_tree_recursively(
                &fullpath,
                &object,
                &mut keyvalues,
                storage,
                strings,
                repository,
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
        strings: &mut StringInterner,
        repository: &ContextKeyValueStore,
    ) -> Result<(), MerkleError> {
        match object {
            Object::Blob(blob_id) => {
                // push key-value pair
                let blob = storage.get_blob(*blob_id)?;
                entries.push((self.string_to_key(path), blob.to_vec()));
                Ok(())
            }
            Object::Directory(dir_id) => {
                let dir = storage.dir_to_vec_unsorted(*dir_id, strings, repository)?;

                dir.iter()
                    .map(|(key, child_dir_entry_id)| {
                        let key = strings.get_str(*key)?;
                        let fullpath = path.to_owned() + "/" + &key;

                        match self.dir_entry_object(*child_dir_entry_id, storage, strings) {
                            Err(_) => Ok(()),
                            Ok(object) => self.collect_key_values_from_tree_recursively(
                                &fullpath, &object, entries, storage, strings, repository,
                            ),
                        }
                    })
                    .find_map(|res| match res {
                        Ok(_) => None,
                        Err(err) => Some(Err(err)),
                    })
                    .unwrap_or(Ok(()))
            }
            Object::Commit(commit) => match self.get_object(commit.root_ref, storage, strings) {
                Err(err) => Err(err),
                Ok(object) => self.collect_key_values_from_tree_recursively(
                    path, &object, entries, storage, strings, repository,
                ),
            },
        }
    }

    /// The repository sometimes needs to have access to the interned strings in `StringInterner`.
    ///
    /// - This method synchronizes the `StringInterner` from the `Storage` into the repository
    ///   when they are differents.
    /// - The repository needs those interned strings when it sends the directory shapes to the
    ///   read only protocol runner.
    fn synchronize_interned_strings_to_repository(&self) -> Result<(), MerkleError> {
        let mut strings = self.get_string_interner()?;
        let mut repository = self.repository.write();
        repository.synchronize_strings_from(&strings);

        strings.all_strings_to_serialize.clear();

        Ok(())
    }

    /// A clone of `Self` but with a clear storage.
    fn with_clear_storage(&self) -> Result<Self, MerkleError> {
        Ok(Self {
            storage: Default::default(),
            patch_context: Rc::clone(&self.patch_context),
            repository: Arc::clone(&self.repository),
            string_interner: Rc::clone(&self.string_interner),
        })
    }

    fn get_key_from_history_impl(
        &self,
        context_hash: &ContextHash,
        key: &ContextKey,
    ) -> Result<Option<ContextValue>, ContextError> {
        let object_ref = {
            let repository = self.repository.write();

            match repository.get_context_hash(context_hash)? {
                Some(hash_id) => hash_id,
                None => {
                    return Err(ContextError::UnknownContextHashError {
                        context_hash: context_hash.to_base58_check(),
                    })
                }
            }
        };

        match self.get_history(object_ref, key) {
            Err(MerkleError::ValueNotFound { key: _ }) => Ok(None),
            Err(MerkleError::ObjectNotFound { .. }) => Ok(None),
            Err(err) => Err(ContextError::MerkleStorageError { error: err }),
            Ok(val) => Ok(Some(val)),
        }
    }

    fn get_key_values_by_prefix_impl(
        &self,
        context_hash: &ContextHash,
        prefix: &ContextKey,
    ) -> Result<Option<Vec<(ContextKeyOwned, ContextValue)>>, ContextError> {
        let object_ref = {
            let repository = self.repository.read();
            match repository.get_context_hash(context_hash)? {
                Some(hash_id) => hash_id,
                None => {
                    return Err(ContextError::UnknownContextHashError {
                        context_hash: context_hash.to_base58_check(),
                    })
                }
            }
        };

        self.get_context_key_values_by_prefix(object_ref, prefix)
            .map_err(Into::into)
    }

    fn get_context_tree_by_prefix_impl(
        &self,
        context_hash: &ContextHash,
        prefix: &ContextKey,
        depth: Option<usize>,
    ) -> Result<StringTreeObject, ContextError> {
        let object_ref = {
            let repository = self.repository.read();
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
        let repository = self.repository.read();
        let mut strings = self.get_string_interner()?;

        self._get_context_tree_by_prefix(
            object_ref,
            prefix,
            depth,
            &mut storage,
            &mut strings,
            &*repository,
        )
        .map_err(ContextError::from)
    }
}

impl IndexApi<TezedgeContext> for TezedgeIndex {
    /// Checks if `context_hash` exists in the repository.
    fn exists(&self, context_hash: &ContextHash) -> Result<bool, ContextError> {
        let object_ref = {
            let repository = self.repository.read();

            match repository.get_context_hash(context_hash)? {
                Some(hash_id) => hash_id,
                None => return Ok(false),
            }
        };

        let mut storage = self.storage.borrow_mut();
        let mut strings = self.get_string_interner()?;

        Ok(self
            .get_commit(object_ref, &mut storage, &mut strings)
            .is_ok())
    }

    fn checkout(&self, context_hash: &ContextHash) -> Result<Option<TezedgeContext>, ContextError> {
        let object_ref = {
            let repository = self.repository.read();

            match repository.get_context_hash(context_hash)? {
                Some(hash_id) => hash_id,
                None => return Ok(None),
            }
        };

        // TODO: should we always be copying this value? is it possibe
        // to keep the latest version around and copy only when not different?
        let index = self.with_clear_storage()?;

        let dir_id = {
            let mut storage = index.storage.borrow_mut();
            let mut strings = index.get_string_interner()?;

            strings.shrink_to_fit();

            let commit = match self.fetch_commit(object_ref, &mut storage, &mut strings)? {
                Some(commit) => commit,
                None => return Ok(None),
            };

            match self.fetch_directory(commit.root_ref, &mut storage, &mut strings)? {
                Some(dir_id) => dir_id,
                None => return Ok(None),
            }
        };

        let tree = WorkingTree::new_with_directory(index.clone(), dir_id);

        Ok(Some(TezedgeContext::new(
            index,
            Some(object_ref),
            Some(Rc::new(tree)),
        )))
    }

    fn latest_context_hashes(&self, count: i64) -> Result<Vec<ContextHash>, ContextError> {
        self.repository
            .read()
            .latest_context_hashes(count)
            .map_err(Into::into)
    }

    fn block_applied(
        &self,
        block_level: u32,
        context_hash: &ContextHash,
    ) -> Result<(), ContextError> {
        Ok(self
            .repository
            .write()
            .block_applied(block_level, context_hash)?)
    }

    fn cycle_started(&mut self) -> Result<(), ContextError> {
        Ok(self.repository.write().new_cycle_started()?)
    }

    fn get_key_from_history(
        &self,
        context_hash: &ContextHash,
        key: &ContextKey,
    ) -> Result<Option<ContextValue>, ContextError> {
        let index = self.with_deallocation();
        index.get_key_from_history_impl(context_hash, key)
    }

    fn get_key_values_by_prefix(
        &self,
        context_hash: &ContextHash,
        prefix: &ContextKey,
    ) -> Result<Option<Vec<(ContextKeyOwned, ContextValue)>>, ContextError> {
        let index = self.with_deallocation();
        index.get_key_values_by_prefix_impl(context_hash, prefix)
    }

    fn get_context_tree_by_prefix(
        &self,
        context_hash: &ContextHash,
        prefix: &ContextKey,
        depth: Option<usize>,
    ) -> Result<StringTreeObject, ContextError> {
        let index = self.with_deallocation();
        index.get_context_tree_by_prefix_impl(context_hash, prefix, depth)
    }
}

/// Handle that represents a specific context (obtained from a checkout).
/// It is a persistent data structure, with each modification producing a new copy.
#[derive(Clone)]
pub struct TezedgeContext {
    /// Index used for fetching and saving objects from/to the repository.
    pub index: TezedgeIndex,
    pub parent_commit_ref: Option<ObjectReference>,
    // NOTE: tree ids are not being used right now, but were used before to
    // identify specific versions of the tree in the context actions replayer.
    pub tree_id: TreeId,
    tree_id_generator: Rc<RefCell<TreeIdGenerator>>,
    /// Root tree for this context handle
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
        order: FoldOrder,
    ) -> Result<TreeWalker, ContextError> {
        Ok(self.tree.fold_iter(depth, key, order)?)
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
        let ctx = self.with_deallocation();
        ctx.commit_impl(author, message, date)
    }

    fn hash(
        &self,
        author: String,
        message: String,
        date: i64,
    ) -> Result<ContextHash, ContextError> {
        let date: u64 = date.try_into()?;
        let mut repository = self.index.repository.write();

        let PostCommitData { commit_ref, .. } = self.tree.prepare_commit(
            date,
            author,
            message,
            self.parent_commit_ref,
            &mut *repository,
            None,
            None,
            false,
        )?;

        get_commit_hash(commit_ref, &*repository)
    }

    fn get_last_commit_hash(&self) -> Result<Option<Vec<u8>>, ContextError> {
        let repository = self.index.repository.read();
        let mut buffer = Vec::with_capacity(1000);

        let object_ref = match self.parent_commit_ref {
            Some(obj_ref) => obj_ref,
            None => return Ok(None),
        };

        repository
            .get_object_bytes(object_ref, &mut buffer)
            .map(|bytes| Some(bytes.to_vec()))
            .map_err(Into::into)
    }

    fn get_memory_usage(&self) -> Result<ContextMemoryUsage, ContextError> {
        let repository = self.index.repository.read();
        let storage = self.index.storage.borrow();
        let strings = self.index.get_string_interner()?;

        let usage = ContextMemoryUsage {
            repo: repository.memory_usage(),
            storage: storage.memory_usage(&strings),
        };

        Ok(usage)
    }
}

// NOTE: right now tree IDs are not used.

/// Generator of Tree IDs which are used to simulate pointers when they are not available.
///
/// During a regular use of the context API, contexts that are still in use are kept
/// alive by pointers to them. This is not available when for example, running the context
/// actions re-player tool. To solve that, we generate a tree id for each version of the
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
        parent_commit_ref: Option<ObjectReference>,
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
            parent_commit_ref,
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

    fn with_deallocation(&self) -> TezedgeContextWithDeallocation<'_> {
        TezedgeContextWithDeallocation { ctx: self }
    }

    fn commit_impl(
        &self,
        author: String,
        message: String,
        date: i64,
    ) -> Result<ContextHash, ContextError> {
        self.index.synchronize_interned_strings_to_repository()?;

        let (commit_hash, serialize_stats) = {
            let mut repository = self.index.repository.write();
            let date: u64 = date.try_into()?;

            repository.commit(&self.tree, self.parent_commit_ref, author, message, date)?
        };

        let mem = self.get_memory_usage()?;

        send_statistics(BlockMemoryUsage {
            context: Box::new(mem),
            serialize: serialize_stats,
        });

        Ok(commit_hash)
    }
}

#[cfg(test)]
mod tests {
    use tezos_context_api::{
        ContextKvStoreConfiguration, TezosContextTezEdgeStorageConfiguration,
        TezosContextTezedgeOnDiskBackendOptions,
    };

    use super::*;
    use crate::initializer::initialize_tezedge_context;

    #[test]
    fn init_context() {
        let context = initialize_tezedge_context(&TezosContextTezEdgeStorageConfiguration {
            backend: ContextKvStoreConfiguration::InMem(TezosContextTezedgeOnDiskBackendOptions {
                base_path: "".to_string(),
                startup_check: false,
            }),
            ipc_socket_path: None,
        })
        .unwrap();

        // Context is immutable so on any modification, the methods return the new tree
        let context = context.add(&["a", "b", "c"], &[1, 2, 3]).unwrap();
        let context = context.add(&["m", "n", "o"], &[4, 5, 6]).unwrap();
        assert_eq!(context.find(&["a", "b", "c"]).unwrap().unwrap(), &[1, 2, 3]);

        let context2 = context.delete(&["m", "n", "o"]).unwrap();
        assert!(context.mem(&["m", "n", "o"]).unwrap());
        assert!(!context2.mem(&["m", "n", "o"]).unwrap());

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
