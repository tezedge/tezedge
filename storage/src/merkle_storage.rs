//! # MerkleStorage
//!
//! Storage for key/values with git-like semantics and history.
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
use std::array::TryFromSliceError;
use std::collections::{BTreeMap, HashMap};
use std::convert::TryInto;
use std::hash::Hash;
use std::sync::Arc;
use std::time::Instant;

use blake2::digest::{Update, VariableOutput};
use blake2::VarBlake2b;
use failure::Fail;
use im::OrdMap;
use rocksdb::{Cache, ColumnFamilyDescriptor, WriteBatch};
use serde::{Deserialize, Serialize};

use crate::persistent;
use crate::persistent::database::RocksDBStats;
use crate::persistent::BincodeEncoded;
use crate::persistent::{default_table_options, KeyValueSchema, KeyValueStoreWithSchema};

const HASH_LEN: usize = 32;

pub type ContextKey = Vec<String>;
pub type ContextValue = Vec<u8>;
pub type EntryHash = [u8; HASH_LEN];

#[derive(Clone, Debug, Serialize, Deserialize)]
enum NodeKind {
    NonLeaf,
    Leaf,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Node {
    node_kind: NodeKind,
    entry_hash: EntryHash,
}

// Tree must be an ordered structure for consistent hash in hash_tree
// Currently immutable OrdMap is used to allow cloning trees without too much overhead
type Tree = OrdMap<String, Node>;

#[derive(Debug, Hash, Clone, Serialize, Deserialize)]
struct Commit {
    parent_commit_hash: Option<EntryHash>,
    root_hash: EntryHash,
    time: u64,
    author: String,
    message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum Entry {
    Tree(Tree),
    Blob(ContextValue),
    Commit(Commit),
}

// impl serde::Serialize for OrdMap<String,Node>{
//     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//     where
//         S: serde::Serializer {
//         let x = self.as_ref();
//         x.serialize(serializer)
//     }
// }

pub type MerkleStorageKV = dyn KeyValueStoreWithSchema<MerkleStorage> + Sync + Send;

pub type RefCnt = usize;

pub struct MerkleStorage {
    /// tree with current staging area (currently checked out context)
    current_stage_tree: (Tree,EntryHash),
    db: Arc<MerkleStorageKV>,
    /// all entries in current staging area
    staged: HashMap<EntryHash, Entry>,
    /// HashMap for looking up entry index in self.staged by hash
    last_commit_hash: Option<EntryHash>,
    /// storage latency statistics
    perf_stats: MerklePerfStats,
}

#[derive(Debug, Fail)]
pub enum MerkleError {
    /// External libs errors
    #[fail(display = "RocksDB error: {:?}", error)]
    DBError {
        error: persistent::database::DBError,
    },
    #[fail(display = "Serialization error: {:?}", error)]
    SerializationError { error: bincode::Error },

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
        display = "Found wrong structure. Was looking for {}, but found {}",
        sought, found
    )]
    FoundUnexpectedStructure { sought: String, found: String },
    #[fail(display = "Entry not found! Hash={}", hash)]
    EntryNotFound { hash: String },
    #[fail(display = "Entry not in staging area! Hash={}", hash)]
    EntryNotFoundInStaging { hash: String },

    /// Wrong user input errors
    #[fail(display = "No value under key {:?}.", key)]
    ValueNotFound { key: String },
    #[fail(display = "Cannot search for an empty key.")]
    KeyEmpty,
    #[fail(display = "Failed to convert hash to array: {}", error)]
    HashConversionError { error: TryFromSliceError },
}

impl From<persistent::database::DBError> for MerkleError {
    fn from(error: persistent::database::DBError) -> Self {
        MerkleError::DBError { error }
    }
}

impl From<bincode::Error> for MerkleError {
    fn from(error: bincode::Error) -> Self {
        MerkleError::SerializationError { error }
    }
}

impl From<TryFromSliceError> for MerkleError {
    fn from(error: TryFromSliceError) -> Self {
        MerkleError::HashConversionError { error }
    }
}

/// Latency statistics for each action (in nanoseconds)
#[derive(Serialize, Debug, Clone, Copy)]
pub struct OperationLatencies {
    /// divide this by the next field to get avg (mean) time spent in operation
    cumul_op_exec_time: f64,
    pub op_exec_times: u64,
    pub avg_exec_time: f64,
    /// lowest time spent in operation
    pub op_exec_time_min: f64,
    /// highest time spent in operation
    pub op_exec_time_max: f64,
}

impl Default for OperationLatencies {
    fn default() -> Self {
        OperationLatencies {
            cumul_op_exec_time: 0.0,
            op_exec_times: 0,
            avg_exec_time: 0.0,
            op_exec_time_min: f64::MAX,
            op_exec_time_max: f64::MIN,
        }
    }
}

// Latency statistics indexed by operation name (e.g. "Set")
pub type OperationLatencyStats = HashMap<String, OperationLatencies>;

// Latency statistics per path indexed by first chunk of path (under /data/)
pub type PerPathOperationStats = HashMap<String, OperationLatencyStats>;

#[derive(Serialize, Debug, Clone)]
pub struct MerklePerfStats {
    pub global: OperationLatencyStats,
    pub perpath: PerPathOperationStats,
}

#[derive(Serialize, Debug, Clone)]
pub struct MerkleStorageStats {
    rocksdb_stats: RocksDBStats,
    pub perf_stats: MerklePerfStats,
}

impl BincodeEncoded for EntryHash {}

impl KeyValueSchema for MerkleStorage {
    // keys is hash of Entry
    type Key = EntryHash;
    // Entry (serialized)
    type Value = Vec<u8>;

    fn descriptor(cache: &Cache) -> ColumnFamilyDescriptor {
        let cf_opts = default_table_options(cache);
        ColumnFamilyDescriptor::new(Self::name(), cf_opts)
    }

    #[inline]
    fn name() -> &'static str {
        "merkle_storage"
    }
}

// Tree in String form needed for JSON RPCs
pub type StringTreeMap = BTreeMap<String, StringTreeEntry>;

/// Tree in String form needed for JSON RPCs
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StringTreeEntry {
    Tree(StringTreeMap),
    Blob(String),
    Null,
}

fn encode_irmin_node_kind(kind: &NodeKind) -> [u8; 8] {
    match kind {
        NodeKind::NonLeaf => [0, 0, 0, 0, 0, 0, 0, 0],
        NodeKind::Leaf => [255, 0, 0, 0, 0, 0, 0, 0],
    }
}

// Calculates hash of tree
// uses BLAKE2 binary 256 length hash function
// hash is calculated as:
// <number of child nodes (8 bytes)><CHILD NODE>
// where:
// - CHILD NODE - <NODE TYPE><length of string (1 byte)><string/path bytes><length of hash (8bytes)><hash bytes>
// - NODE TYPE - leaf node(0xff0000000000000000) or internal node (0x0000000000000000)
fn hash_tree(tree: &Tree) -> EntryHash {
    let mut hasher = VarBlake2b::new(HASH_LEN).unwrap();

    hasher.update(&(tree.len() as u64).to_be_bytes());
    tree.iter().for_each(|(k, v)| {
        hasher.update(encode_irmin_node_kind(&v.node_kind));
        hasher.update(&[k.len() as u8]);
        hasher.update(&k.clone().into_bytes());
        hasher.update(&(HASH_LEN as u64).to_be_bytes());
        hasher.update(&v.entry_hash);
    });
    hasher.finalize_boxed().as_ref().try_into().unwrap()
}

// Calculates hash of BLOB
// uses BLAKE2 binary 256 length hash function
// hash is calculated as <length of data (8 bytes)><data>
fn hash_blob(blob: &ContextValue) -> EntryHash {
    let mut hasher = VarBlake2b::new(HASH_LEN).unwrap();
    hasher.update(&(blob.len() as u64).to_be_bytes());
    hasher.update(blob);

    hasher.finalize_boxed().as_ref().try_into().unwrap()
}

// Calculates hash of commit
// uses BLAKE2 binary 256 length hash function
// hash is calculated as:
// <hash length (8 bytes)><tree hash bytes>
// <length of parent hash (8bytes)><parent hash bytes>
// <time in epoch format (8bytes)
// <commit author name length (8bytes)><commit author name bytes>
// <commit message length (8bytes)><commit message bytes>
fn hash_commit(commit: &Commit) -> EntryHash {
    let mut hasher = VarBlake2b::new(HASH_LEN).unwrap();
    hasher.update(&(HASH_LEN as u64).to_be_bytes());
    hasher.update(&commit.root_hash);

    if commit.parent_commit_hash.is_none() {
        hasher.update(&(0_u64).to_be_bytes());
    } else {
        hasher.update(&(1_u64).to_be_bytes()); // # of parents; we support only 1
        hasher.update(&(commit.parent_commit_hash.unwrap().len() as u64).to_be_bytes());
        hasher.update(&commit.parent_commit_hash.unwrap());
    }
    hasher.update(&(commit.time as u64).to_be_bytes());
    hasher.update(&(commit.author.len() as u64).to_be_bytes());
    hasher.update(&commit.author.clone().into_bytes());
    hasher.update(&(commit.message.len() as u64).to_be_bytes());
    hasher.update(&commit.message.clone().into_bytes());

    hasher.finalize_boxed().as_ref().try_into().unwrap()
}

impl MerkleStorage {
    pub fn new(db: Arc<MerkleStorageKV>) -> Self {
        let tree = Tree::new();
        let tree_hash = hash_tree(&tree);
        let mut map: HashMap<EntryHash, Entry> = HashMap::new();
        map.insert(tree_hash,Entry::Tree(tree.clone()));

        MerkleStorage {
            db,
            staged: map,
            current_stage_tree: (tree,tree_hash),
            last_commit_hash: None,
            perf_stats: MerklePerfStats {
                global: HashMap::new(),
                perpath: HashMap::new(),
            },
        }
    }

    /// Get value from current staged root
    pub fn get(&mut self, key: &ContextKey) -> Result<ContextValue, MerkleError> {
        let instant = Instant::now();
        // build staging tree from saved list of actions (set/copy/delete)
        // note: this can be slow if there are a lot of actions
        let root = &self.get_staged_root();
        let root_hash = hash_tree(&root);

        let rv = self.get_from_tree(&root_hash, key);
        self.update_execution_stats("Get".to_string(), Some(&key), &instant);
        if rv.is_err() {
            Ok(Vec::new())
        } else {
            rv
        }
    }

    /// Check if value exists in current staged root
    pub fn mem(&mut self, key: &ContextKey) -> Result<bool, MerkleError> {
        let instant = Instant::now();
        // build staging tree from saved list of actions (set/copy/delete)
        // note: this can be slow if there are a lot of actions

        let root = &self.get_staged_root();
        let root_hash = hash_tree(&root);

        let rv = self.value_exists(&root_hash, key);
        self.update_execution_stats("Mem".to_string(), Some(&key), &instant);
        rv
    }

    /// Check if directory exists in current staged root
    pub fn dirmem(&mut self, key: &ContextKey) -> Result<bool, MerkleError> {
        // build staging tree from saved list of actions (set/copy/delete)
        // note: this can be slow if there are a lot of actions

        let root = &self.get_staged_root();
        let root_hash = hash_tree(&root);

        let rv = self.directory_exists(&root_hash, key);
        // self.update_execution_stats("DirMem".to_string(), Some(&key), &instant);
        rv
    }

    /// Get value. Staging area is checked first, then last (checked out) commit.
    pub fn get_by_prefix(
        &mut self,
        prefix: &ContextKey,
    ) -> Result<Option<Vec<(ContextKey, ContextValue)>>, MerkleError> {
        let root = self.get_staged_root();
        self._get_key_values_by_prefix(root.clone(), prefix)
    }

    /// Get value from historical context identified by commit hash.
    pub fn get_history(
        &mut self,
        commit_hash: &EntryHash,
        key: &ContextKey,
    ) -> Result<ContextValue, MerkleError> {
        let instant = Instant::now();
        let commit = self.get_commit(commit_hash)?;

        let rv = self.get_from_tree(&commit.root_hash, key);
        self.update_execution_stats("GetKeyFromHistory".to_string(), Some(&key), &instant);
        rv
    }

    fn value_exists(&self, root_hash: &EntryHash, key: &ContextKey) -> Result<bool, MerkleError> {
        let mut full_path = key.clone();
        let file = full_path.pop().ok_or(MerkleError::KeyEmpty)?;
        let path = full_path;
        // find tree by path
        let root = self.get_tree(root_hash)?;
        let node = self.find_tree(&root, &path);
        if node.is_err() {
            return Ok(false);
        }

        // get file node from tree
        if node?.get(&file).is_some() {
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn directory_exists(
        &self,
        root_hash: &EntryHash,
        key: &ContextKey,
    ) -> Result<bool, MerkleError> {
        // find tree by path
        let root = self.get_tree(root_hash)?;
        let node = self.find_tree(&root, &key);
        if node.is_err() || node?.is_empty() {
            Ok(false)
        } else {
            Ok(true)
        }
    }

    fn get_from_tree(
        &self,
        root_hash: &EntryHash,
        key: &ContextKey,
    ) -> Result<ContextValue, MerkleError> {
        let mut full_path = key.clone();
        let file = full_path.pop().ok_or(MerkleError::KeyEmpty)?;
        let path = full_path;
        // find tree by path
        let root = self.get_tree(root_hash)?;
        let node = self.find_tree(&root, &path)?;

        // get file node from tree
        let node = match node.get(&file) {
            None => {
                return Err(MerkleError::ValueNotFound {
                    key: self.key_to_string(key),
                })
            }
            Some(entry) => entry,
        };
        // get blob by hash
        match self.get_entry(&node.entry_hash)? {
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
                entries.push((self.string_to_key(path), blob.to_vec()));
                Ok(())
            }
            Entry::Tree(tree) => {
                // Go through all descendants and gather errors. Remap error if there is a failure
                // anywhere in the recursion paths. TODO: is revert possible?
                tree.iter()
                    .map(|(key, child_node)| {
                        let fullpath = path.to_owned() + "/" + key;
                        match self.get_entry(&child_node.entry_hash) {
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
            Entry::Commit(commit) => match self.get_entry(&commit.root_hash) {
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
                    let e = self.get_entry(&child_node.entry_hash)?;
                    let rdepth = depth.map(|d| d - 1);
                    new_tree.insert(
                        key.to_owned(),
                        self.get_context_recursive(&fullpath, &e, rdepth)?,
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

        let instant = Instant::now();
        let mut out = StringTreeMap::new();
        let commit = self.get_commit(context_hash)?;
        let root_tree = self.get_tree(&commit.root_hash)?;
        let prefixed_tree = self.find_tree(&root_tree, prefix)?;

        for (key, child_node) in prefixed_tree.iter() {
            let entry = self.get_entry(&child_node.entry_hash)?;
            let delimiter: &str;
            if prefix.is_empty() {
                delimiter = "";
            } else {
                delimiter = "/";
            }

            // construct full path as Tree key is only one chunk of it
            let fullpath = self.key_to_string(prefix) + delimiter + key;
            let rdepth = depth.map(|d| d - 1);
            out.insert(
                key.to_owned(),
                self.get_context_recursive(&fullpath, &entry, rdepth)?,
            );
        }

        self.update_execution_stats(
            "GetContextTreeByPrefix".to_string(),
            Some(&prefix),
            &instant,
        );
        Ok(StringTreeEntry::Tree(out))
    }

    /// Construct Vec of all context key-values under given prefix
    pub fn get_key_values_by_prefix(
        &mut self,
        context_hash: &EntryHash,
        prefix: &ContextKey,
    ) -> Result<Option<Vec<(ContextKey, ContextValue)>>, MerkleError> {
        let instant = Instant::now();
        let commit = self.get_commit(context_hash)?;
        let root_tree = self.get_tree(&commit.root_hash)?;
        let rv = self._get_key_values_by_prefix(root_tree, prefix);
        self.update_execution_stats("GetKeyValuesByPrefix".to_string(), Some(&prefix), &instant);
        rv
    }

    fn _get_key_values_by_prefix(
        &self,
        root_tree: Tree,
        prefix: &ContextKey,
    ) -> Result<Option<Vec<(ContextKey, ContextValue)>>, MerkleError> {
        let prefixed_tree = self.find_tree(&root_tree, prefix)?;
        let mut keyvalues: Vec<(ContextKey, ContextValue)> = Vec::new();

        for (key, child_node) in prefixed_tree.iter() {
            let entry = self.get_entry(&child_node.entry_hash)?;
            let delimiter: &str;
            if prefix.is_empty() {
                delimiter = "";
            } else {
                delimiter = "/";
            }
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

    /// Flush the staging area and and move to work on a certain commit from history.
    pub fn checkout(&mut self, context_hash: &EntryHash) -> Result<(), MerkleError> {
        let commit = self.get_commit(&context_hash)?;
        let tree = self.get_tree(&commit.root_hash)?;
        self.store_current_tree_root(&tree);
        self.last_commit_hash = Some(hash_commit(&commit));
        self.staged.clear();
        Ok(())
    }

    /// Take the current changes in the staging area, create a commit and persist all changes
    /// to database under the new commit. Return last commit if there are no changes, that is
    /// empty commits are not allowed.
    pub fn commit(
        &mut self,
        time: u64,
        author: String,
        message: String,
    ) -> Result<EntryHash, MerkleError> {
        let staged_root = self.get_staged_root();
        let staged_root_hash = hash_tree(&staged_root);
        let parent_commit_hash = self.last_commit_hash;

        let new_commit = Commit {
            root_hash: staged_root_hash,
            parent_commit_hash,
            time,
            author,
            message,
        };
        let entry = Entry::Commit(new_commit.clone());
        self.put_to_staging_area(&hash_commit(&new_commit), entry.clone());
        self.persist_staged_entry_to_db(&entry)?;

        self.staged.clear();
        self.last_commit_hash = Some(hash_commit(&new_commit.clone()));
        Ok(hash_commit(&new_commit))
    }

    /// Set key/val to the staging area.
    fn store_current_tree_root(&mut self, tree: &Tree)  {
        self.current_stage_tree = (tree.clone(), hash_tree(&tree));
    }

    /// Set key/val to the staging area.
    pub fn set(&mut self, key: &ContextKey, value: &ContextValue) -> Result<(), MerkleError> {
        let root = self.get_staged_root();
        let new_root_hash = &self._set(&root, key, value)?;
        self.store_current_tree_root(&self.get_tree(new_root_hash)?);
        Ok(())
    }

    fn _set(
        &mut self,
        root: &Tree,
        key: &ContextKey,
        value: &ContextValue,
    ) -> Result<EntryHash, MerkleError> {
        let blob_hash = hash_blob(&value);
        self.put_to_staging_area(&blob_hash, Entry::Blob(value.clone()));
        let new_node = Node {
            entry_hash: blob_hash,
            node_kind: NodeKind::Leaf,
        };
        let rv = self.compute_new_root_with_change(root, &key, Some(new_node));
        rv
    }

    /// Delete an item from the staging area.
    //TODO: pelight
    pub fn delete(&mut self, key: &ContextKey) -> Result<(), MerkleError> {
        let root = self.get_staged_root();
        let new_root_hash = &self._delete(&root, key)?;
        let tree = self.get_tree(new_root_hash)?;
        self.store_current_tree_root(&tree);

        Ok(())
    }

    fn _delete(&mut self, root: &Tree, key: &ContextKey) -> Result<EntryHash, MerkleError> {
        if key.is_empty() {
            return Ok(hash_tree(root));
        }
        self.compute_new_root_with_change(root, &key, None)
    }

    /// Copy subtree under a new path.
    /// TODO Consider copying values!
    pub fn copy(&mut self, from_key: &ContextKey, to_key: &ContextKey) -> Result<(), MerkleError> {
        let root = self.get_staged_root();
        let new_root_hash = self._copy(&root, from_key, to_key)?;
        self.store_current_tree_root(&self.get_tree(&new_root_hash)?);
        Ok(())
    }

    fn _copy(
        &mut self,
        root: &Tree,
        from_key: &ContextKey,
        to_key: &ContextKey,
    ) -> Result<EntryHash, MerkleError> {
        let source_tree = self.find_tree(root, &from_key)?;
        let source_tree_hash = hash_tree(&source_tree);
        Ok(self.compute_new_root_with_change(
            &root,
            &to_key,
            Some(self.get_non_leaf(source_tree_hash)),
        )?)
    }

    /// Get a new tree with `new_node` put under given `key`.
    /// Walk down the tree to find key, set new value and walk back up recalculating hashes -
    /// return new top hash of tree. Note: no writes to DB yet
    ///
    /// # Arguments
    ///
    /// * `root_hash` - hash of Tree to modify
    /// * `key` - path under which the changes takes place
    /// * `new_node` - None for deletion, Some for inserting a hash under the key.
    /// Get a new tree with `new_entry_hash` put under given `key`.
    ///
    /// # Arguments
    ///
    /// * `root` - Tree to modify
    /// * `key` - path under which the changes takes place
    /// * `new_entry_hash` - None for deletion, Some for inserting a hash under the key.
    fn compute_new_root_with_change(
        &mut self,
        root: &Tree,
        key: &[String],
        new_node: Option<Node>,
    ) -> Result<EntryHash, MerkleError> {
        if key.is_empty() {
            match new_node {
                Some(n) => {
                    return Ok(n.entry_hash);
                }
                None => {
                    let tree = Tree::new();
                    let new_tree_hash = hash_tree(&tree);
                    self.put_to_staging_area(&new_tree_hash, Entry::Tree(tree));
                    return Ok(new_tree_hash);
                }
            }
        }

        let last = key.last().unwrap();
        let path = &key[..key.len() - 1];
        let mut tree = self.find_tree(root, path)?;

        match new_node {
            None => tree.remove(last),
            Some(new_node) => tree.insert(last.clone(), new_node),
        };

        if tree.is_empty() {
            self.compute_new_root_with_change(root, path, None)
        } else {
            let new_tree_hash = hash_tree(&tree);
            self.put_to_staging_area(&new_tree_hash, Entry::Tree(tree));
            self.compute_new_root_with_change(root, path, Some(self.get_non_leaf(new_tree_hash)))
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
        // terminate recursion if end of path was reached
        if key.is_empty() {
            return Ok(root.clone());
        }

        // first get node at key
        let child_node = match root.get(key.first().unwrap()) {
            Some(hash) => hash,
            None => {
                return Ok(Tree::new());
            }
        };

        // get entry by hash (from staged area or DB)
        match self.get_entry(&child_node.entry_hash)? {
            Entry::Tree(tree) => self.find_tree(&tree, &key[1..]),
            Entry::Blob(_) => Ok(Tree::new()),
            Entry::Commit { .. } => Err(MerkleError::FoundUnexpectedStructure {
                sought: "Tree/Blob".to_string(),
                found: "commit".to_string(),
            }),
        }
    }

    /// Get latest staged tree. If it's empty, init genesis  and return genesis root.
    fn get_staged_root(&mut self) -> Tree {
        self.current_stage_tree.0.clone()
    }

    /// Put entry in staging area
    fn put_to_staging_area(&mut self, key: &EntryHash, value: Entry) {
        self.staged.insert(*key, value);
    }

    /// Persists an entry and its descendants from staged area to database on disk.
    fn persist_staged_entry_to_db(&self, entry: &Entry) -> Result<(), MerkleError> {
        let mut batch = WriteBatch::default(); // batch containing DB key values to persist

        // build list of entries to be persisted
        self.get_entries_recursively(entry, &mut batch)?;

        // atomically write all entries in one batch to DB
        self.db.write_batch(batch)?;

        Ok(())
    }

    /// Builds vector of entries to be persisted to DB, recursively
    fn get_entries_recursively(
        &self,
        entry: &Entry,
        batch: &mut WriteBatch,
    ) -> Result<(), MerkleError> {
        //passing entry by reference is tricky as its recursive function...
        self.db
            .put_batch(batch, &self.hash_entry(entry), &bincode::serialize(entry)?)?;

        match entry {
            Entry::Blob(_) => Ok(()),
            Entry::Tree(tree) => {
                // Go through all descendants and gather errors. Remap error if there is a failure
                // anywhere in the recursion paths. TODO: is revert possible?
                tree.iter()
                    .map(
                        |(_, child_node)| match self.staged.get(&child_node.entry_hash) {
                            None => Ok(()),
                            Some(entry) => self.get_entries_recursively(entry, batch),
                        },
                    )
                    .find_map(|res| match res {
                        Ok(_) => None,
                        Err(err) => Some(Err(err)),
                    })
                    .unwrap_or(Ok(()))
            }
            Entry::Commit(commit) => match self.get_entry(&commit.root_hash) {
                Err(err) => Err(err),
                Ok(entry) => self.get_entries_recursively(&entry, batch),
            },
        }
    }

    fn hash_entry(&self, entry: &Entry) -> EntryHash {
        match entry {
            Entry::Commit(commit) => hash_commit(&commit),
            Entry::Tree(tree) => hash_tree(&tree),
            Entry::Blob(blob) => hash_blob(blob),
        }
    }

    fn get_tree(&self, hash: &EntryHash) -> Result<Tree, MerkleError> {
        match self.get_entry(hash)? {
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
        match self.get_entry(hash)? {
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

    fn get_entry(&self, hash: &EntryHash) -> Result<Entry, MerkleError> {
        match self.staged.get(hash) {
            None => {
                let entry_bytes = self.db.get(hash)?;
                match entry_bytes {
                    None => Err(MerkleError::EntryNotFound {
                        hash: hex::encode(hash),
                    }),
                    Some(entry_bytes) => Ok(bincode::deserialize(&entry_bytes)?),
                }
            }
            Some(entry) => Ok(entry.clone()),
        }
    }

    fn get_non_leaf(&self, hash: EntryHash) -> Node {
        Node {
            node_kind: NodeKind::NonLeaf,
            entry_hash: hash,
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

    pub fn get_staged_entries(&self) -> std::string::String {
        let mut result = String::new();
        for (hash, entry) in &self.staged {
            match entry {
                Entry::Blob(blob) => {
                    result += &format!("{}: Value {:?}, \n", hex::encode(&hash[0..3]), blob);
                }

                Entry::Tree(tree) => {
                    if tree.is_empty() {
                        continue;
                    }
                    let tree_hash = &hash_tree(&tree)[0..3];
                    result += &format!("{}: Tree {{", hex::encode(tree_hash));

                    for (path, val) in tree {
                        let kind = if let NodeKind::NonLeaf = val.node_kind {
                            "Tree"
                        } else {
                            "Value/Leaf"
                        };
                        result += &format!(
                            "{}: {}({:?}), ",
                            path,
                            kind,
                            hex::encode(&val.entry_hash[0..3])
                        );
                    }
                    result += "}}\n";
                }

                Entry::Commit(_) => {
                    panic!("commits must not occur in staged area");
                }
            }
        }
        result
    }

    /// Get various merkle storage statistics
    pub fn get_merkle_stats(&self) -> Result<MerkleStorageStats, MerkleError> {
        let db_stats = self.db.get_mem_use_stats()?;

        // calculate average values for global stats
        let mut perf = self.perf_stats.clone();
        for (_, stat) in perf.global.iter_mut() {
            if stat.op_exec_times > 0 {
                stat.avg_exec_time = stat.cumul_op_exec_time / (stat.op_exec_times as f64);
            } else {
                stat.avg_exec_time = 0.0;
            }
        }
        // calculate average values for per-path stats
        for (_node, stat) in perf.perpath.iter_mut() {
            for (_op, stat) in stat.iter_mut() {
                if stat.op_exec_times > 0 {
                    stat.avg_exec_time = stat.cumul_op_exec_time / (stat.op_exec_times as f64);
                } else {
                    stat.avg_exec_time = 0.0;
                }
            }
        }
        Ok(MerkleStorageStats {
            rocksdb_stats: db_stats,
            perf_stats: perf,
        })
    }

    /// Update global and per-path execution stats. Pass Instant with operation execution time
    pub fn update_execution_stats(
        &mut self,
        op: String,
        path: Option<&ContextKey>,
        instant: &Instant,
    ) {
        // stop timer and get duration
        let exec_time: f64 = instant.elapsed().as_nanos() as f64;

        // collect global stats
        let entry = self
            .perf_stats
            .global
            .entry(op.to_owned())
            .or_insert_with(OperationLatencies::default);
        // add to cumulative execution time
        entry.cumul_op_exec_time += exec_time;
        entry.op_exec_times += 1;

        // update min/max times for op
        if exec_time < entry.op_exec_time_min {
            entry.op_exec_time_min = exec_time;
        }
        if exec_time > entry.op_exec_time_max {
            entry.op_exec_time_max = exec_time;
        }

        // collect per-path stats
        if let Some(path) = path {
            // we are only interested in nodes under /data
            if path.len() > 1 && path[0] == "data" {
                let node = path[1].to_string();
                let perpath = self
                    .perf_stats
                    .perpath
                    .entry(node)
                    .or_insert_with(HashMap::new);
                let entry = perpath
                    .entry(op)
                    .or_insert_with(OperationLatencies::default);

                // add to cumulative execution time
                entry.cumul_op_exec_time += exec_time;
                entry.op_exec_times += 1;

                // update min/max times for op
                if exec_time < entry.op_exec_time_min {
                    entry.op_exec_time_min = exec_time;
                }
                if exec_time > entry.op_exec_time_max {
                    entry.op_exec_time_max = exec_time;
                }
            }
        }
    }
}

#[cfg(test)]
#[allow(unused_must_use)]
mod tests {
    // use hex;
    use assert_json_diff::assert_json_eq;
    use rocksdb::{Options, DB};
    use std::path::{Path, PathBuf};
    use std::{env, fs};

    use super::*;

    /// Open DB at path, used in tests
    fn open_db<P: AsRef<Path>>(path: P, cache: &Cache) -> DB {
        let mut db_opts = Options::default();
        db_opts.create_if_missing(true);
        db_opts.create_missing_column_families(true);

        DB::open_cf_descriptors(&db_opts, path, vec![MerkleStorage::descriptor(&cache)]).unwrap()
    }

    pub fn out_dir_path(dir_name: &str) -> PathBuf {
        let out_dir = env::var("OUT_DIR").expect("OUT_DIR is not defined");
        Path::new(out_dir.as_str()).join(Path::new(dir_name))
    }

    fn get_db_name(db_name: &str) -> PathBuf {
        out_dir_path(db_name)
    }

    fn get_db(db_name: &str, cache: &Cache) -> DB {
        open_db(get_db_name(db_name), &cache)
    }

    fn get_storage(dn_name: &str, cache: &Cache) -> MerkleStorage {
        MerkleStorage::new(Arc::new(get_db(dn_name, &cache)))
    }

    fn clean_db(db_name: &str) {
        let _ = DB::destroy(&Options::default(), get_db_name(db_name));
        let _ = fs::remove_dir_all(get_db_name(db_name));
    }

    #[test]
    fn test_duplicate_entry_in_staging() {
        let cache = Cache::new_lru_cache(32 * 1024 * 1024).unwrap();
        let mut storage = get_storage("ms_test_duplicate_entry", &cache);
        let a_foo: &ContextKey = &vec!["a".to_string(), "foo".to_string()];
        let c_foo: &ContextKey = &vec!["c".to_string(), "foo".to_string()];
        storage.set(&vec!["a".to_string(), "foo".to_string()], &vec![97, 98]);
        storage.set(&vec!["c".to_string(), "zoo".to_string()], &vec![1, 2]);
        storage.set(&vec!["c".to_string(), "foo".to_string()], &vec![97, 98]);
        storage.delete(&vec!["c".to_string(), "zoo".to_string()]);
        // now c/ is the same tree as a/ - which means there are two references to single entry in staging area
        // modify the tree and check that the other one was kept intact
        storage.set(&vec!["c".to_string(), "foo".to_string()], &vec![3, 4]);
        let commit = storage
            .commit(0, "Tezos".to_string(), "Genesis".to_string())
            .unwrap();
        assert_eq!(storage.get_history(&commit, a_foo).unwrap(), vec![97, 98]);
        assert_eq!(storage.get_history(&commit, c_foo).unwrap(), vec![3, 4]);
    }

    #[test]
    fn test_hash_of_value_1_blob() {
        let expected_hash =
            "407f958990678e2e9fb06758bc6520dae46d838d39948a4c51a5b19bd079293d".to_string();

        // hash is calculated as <length of data (8 bytes)><data>
        let blob = vec![1];

        // hexademical representation of above commit:
        //
        // value length (8 bytes)          ->  00 00 00 00 00 00 00 01
        // value bytes                     ->  01

        let mut bytes = String::new();
        let value_length = "0000000000000001";
        let value = "01";

        println!("calculating hash of value: {:?}\n", blob);

        println!("[hex] value_length : {}", value_length);
        println!("[hex] value : {}", value);

        bytes += &value_length;
        bytes += &value;

        println!(
            "manually calculated haxedemical representation of value/blob: {}",
            bytes
        );

        let mut hasher = VarBlake2b::new(HASH_LEN).unwrap();
        hasher.update(hex::decode(bytes).unwrap());
        let calcualted_hash = hex::encode(hasher.finalize_boxed().as_ref());

        println!("calculated hash of the value/blob: {}", calcualted_hash);

        assert_eq!(expected_hash, calcualted_hash);
        assert_eq!(hex::encode(hash_blob(&blob)), calcualted_hash);
    }

    #[test]
    fn test_hash_of_commit() {
        // Calculates hash of commit
        // uses BLAKE2 binary 256 length hash function
        // hash is calculated as:
        // <hash length (8 bytes)><tree hash bytes>
        // <length of parent hash (8bytes)><parent hash bytes>
        // <time in epoch format (8bytes)
        // <commit author name length (8bytes)><commit author name bytes>
        // <commit message length (8bytes)><commit message bytes>
        let expected_commit_hash =
            "e6de3fd37b1dc2b3c9d072ea67c2c5be1b55eeed9f5377b2bfc1228e6f9cb69b";
        let dummy_commit = Commit {
            parent_commit_hash: None,
            root_hash: hex::decode(
                "0d78b30e959c2a079e8ccb4ca19d428c95d29b2f02a35c1c58ef9c8972bc26aa",
            )
            .unwrap()
            .try_into()
            .unwrap(),
            time: 0,
            author: "Tezedge".to_string(),
            message: "persist changes".to_string(),
        };

        // hexademical representation of above commit:
        //
        // hash length (8 bytes)           ->  00 00 00 00 00 00 00 20
        // tree hash bytes                 ->  0d78b30e959c2a079e8ccb4ca19d428c95d29b2f02a35c1c58ef9c8972bc26aa
        // parents count                   ->  00 00 00 00 00 00 00 00  (0)
        // commit time                     ->  00 00 00 00 00 00 00 00  (0)
        // commit author name length       ->  00 00 00 00 00 00 00 07  (7)
        // commit author name ('Tezedge')  ->  54 65 7a 65 64 67 65     (Tezedge)
        // commit message length           ->  00 00 00 00 00 00 00 0xf (15)

        let mut bytes = String::new();
        let hash_length = "0000000000000020"; // 32
        let tree_hash = "0d78b30e959c2a079e8ccb4ca19d428c95d29b2f02a35c1c58ef9c8972bc26aa"; // tree hash bytes
        let parents_count = "0000000000000000"; // 0
        let commit_time = "0000000000000000"; // 0
        let commit_author_name_length = "0000000000000007"; // 7
        let commit_author_name = "54657a65646765"; // 'Tezedge'
        let commit_message_length = "000000000000000f"; // 15
        let commit_message = "70657273697374206368616e676573"; // 'persist changes'

        println!("calculating hash of commit: \n\t{:?}\n", dummy_commit);

        println!("[hex] hash_length : {}", hash_length);
        println!("[hex] tree_hash : {}", tree_hash);
        println!("[hex] parents_count : {}", parents_count);
        println!("[hex] commit_time : {}", commit_time);
        println!(
            "[hex] commit_author_name_length : {}",
            commit_author_name_length
        );
        println!("[hex] commit_author_name : {}", commit_author_name);
        println!("[hex] commit_message_length : {}", commit_message_length);
        println!("[hex] commit_message : {}", commit_message);

        bytes += &hash_length;
        bytes += &tree_hash;
        bytes += &parents_count;
        bytes += &commit_time;
        bytes += &commit_author_name_length;
        bytes += &commit_author_name;
        bytes += &commit_message_length;
        bytes += &commit_message;

        println!(
            "manually calculated haxedemical representation of commit: {}",
            bytes
        );

        let mut hasher = VarBlake2b::new(HASH_LEN).unwrap();
        hasher.update(hex::decode(bytes).unwrap());
        let calculated_commit_hash = hasher.finalize_boxed();

        println!(
            "calculated hash of the commit: {}",
            hex::encode(calculated_commit_hash.as_ref())
        );

        assert_eq!(calculated_commit_hash.as_ref(), hash_commit(&dummy_commit));
        assert_eq!(
            expected_commit_hash,
            hex::encode(calculated_commit_hash.as_ref())
        );
    }

    #[test]
    fn test_hash_of_small_tree() {
        // Calculates hash of tree
        // uses BLAKE2 binary 256 length hash function
        // hash is calculated as:
        // <number of child nodes (8 bytes)><CHILD NODE>
        // where:
        // - CHILD NODE - <NODE TYPE><length of string (1 byte)><string/path bytes><length of hash (8bytes)><hash bytes>
        // - NODE TYPE - leaf node(0xff00000000000000) or internal node (0x0000000000000000)

        let expected_tree_hash = "d49a53323107f2ae40b01eaa4e9bec4d02801daf60bab82dc2529e40d40fa917";
        let mut dummy_tree = Tree::new();
        let node = Node {
            node_kind: NodeKind::Leaf,
            entry_hash: hash_blob(&vec![1]), // 407f958990678e2e9fb06758bc6520dae46d838d39948a4c51a5b19bd079293d
        };
        dummy_tree.insert("a".to_string(), node);

        // hexademical representation of above tree:
        //
        // number of child nodes           ->  00 00 00 00 00 00 00 01  (1)
        // node type                       ->  ff 00 00 00 00 00 00 00  (leaf node)
        // length of string                ->  01                       (1)
        // string                          ->  61                       ('a')
        // length of hash                  ->  00 00 00 00 00 00 00 20  (32)
        // hash                            ->  407f958990678e2e9fb06758bc6520dae46d838d39948a4c51a5b19bd079293d

        let mut bytes = String::new();
        let child_nodes = "0000000000000001";
        let leaf_node = "ff00000000000000";
        let string_length = "01";
        let string_value = "61";
        let hash_length = "0000000000000020";
        let hash = "407f958990678e2e9fb06758bc6520dae46d838d39948a4c51a5b19bd079293d";

        println!("calculating hash of tree: \n\t{:?}\n", dummy_tree);
        println!("[hex] child nodes count: {}", child_nodes);
        println!("[hex] leaf_node        : {}", leaf_node);
        println!("[hex] string_length    : {}", string_length);
        println!("[hex] string_value     : {}", string_value);
        println!("[hex] hash_length      : {}", hash_length);
        println!("[hex] hash             : {}", hash);

        bytes += &child_nodes;
        bytes += &leaf_node;
        bytes += &string_length;
        bytes += &string_value;
        bytes += &hash_length;
        bytes += &hash;

        println!(
            "manually calculated haxedemical representation of tree: {}",
            bytes
        );

        let mut hasher = VarBlake2b::new(HASH_LEN).unwrap();
        hasher.update(hex::decode(bytes).unwrap());
        let calculated_tree_hash = hasher.finalize_boxed();

        println!(
            "calculated hash of the tree: {}",
            hex::encode(calculated_tree_hash.as_ref())
        );

        assert_eq!(calculated_tree_hash.as_ref(), hash_tree(&dummy_tree));
        assert_eq!(
            calculated_tree_hash.as_ref(),
            hex::decode(expected_tree_hash).unwrap()
        );
    }

    #[test]
    fn test_tree_hash() {
        let cache = Cache::new_lru_cache(32 * 1024 * 1024).unwrap();
        let mut storage = get_storage("ms_test_tree_hash", &cache);
        storage.set(&vec!["a".to_string(), "foo".to_string()], &vec![97, 98, 99]); // abc
        storage.set(&vec!["b".to_string(), "boo".to_string()], &vec![97, 98]);
        storage.set(
            &vec!["a".to_string(), "aaa".to_string()],
            &vec![97, 98, 99, 100],
        );
        storage.set(&vec!["x".to_string()], &vec![97]);
        storage.set(
            &vec!["one".to_string(), "two".to_string(), "three".to_string()],
            &vec![97],
        );
        storage.commit(0, "Tezos".to_string(), "Genesis".to_string());

        let tree = storage.get_staged_root();

        let hash = hash_tree(&tree);

        assert_eq!([0xDB, 0xAE, 0xD7, 0xB6], hash[0..4]);
    }

    #[test]
    fn test_commit_hash() {
        let cache = Cache::new_lru_cache(32 * 1024 * 1024).unwrap();
        let mut storage = get_storage("ms_test_commit_hash", &cache);
        storage.set(&vec!["a".to_string()], &vec![97, 98, 99]);

        let commit = storage.commit(0, "Tezos".to_string(), "Genesis".to_string());

        assert_eq!([0xCF, 0x95, 0x18, 0x33], commit.unwrap()[0..4]);

        storage.set(&vec!["data".to_string(), "x".to_string()], &vec![97]);
        let commit = storage.commit(0, "Tezos".to_string(), "".to_string());

        assert_eq!([0xCA, 0x7B, 0xC7, 0x02], commit.unwrap()[0..4]);
        // full irmin hash: ca7bc7022ffbd35acc97f7defb00c486bb7f4d19a2d62790d5949775eb74f3c8
    }

    fn get_short_hash(hash: &EntryHash) -> String {
        hex::encode(&hash[0..3])
    }

    fn get_staged_root_short_hash(storage: &mut MerkleStorage) -> String {
        let tree = storage.get_staged_root();
        let hash = hash_tree(&tree);
        get_short_hash(&hash)
    }

    #[test]
    fn test_examples_from_article_about_storage() {
        let cache = Cache::new_lru_cache(32 * 1024 * 1024).unwrap();
        let db_name = "test_examples_from_article_about_storage";
        clean_db(db_name);
        let mut storage = get_storage(db_name, &cache);

        storage.set(&vec!["a".to_string()], &vec![1]);
        let root = get_staged_root_short_hash(&mut storage);
        println!("SET [a] = 1\nROOT: {}", root);
        println!("CONTENT {}", storage.get_staged_entries());
        assert_eq!(root, "d49a53".to_string());

        storage.set(&vec!["b".to_string(), "c".to_string()], &vec![1]);
        let root = get_staged_root_short_hash(&mut storage);
        println!("\nSET [b,c] = 1\nROOT: {}", root);
        print!("{}", storage.get_staged_entries());
        assert_eq!(root, "ed8adf".to_string());

        storage.set(&vec!["b".to_string(), "d".to_string()], &vec![2]);
        let root = get_staged_root_short_hash(&mut storage);
        println!("\nSET [b,d] = 2\nROOT: {}", root);
        print!("{}", storage.get_staged_entries());
        assert_eq!(root, "437186".to_string());

        storage.set(&vec!["a".to_string()], &vec![2]);
        let root = get_staged_root_short_hash(&mut storage);
        println!("\nSET [a] = 2\nROOT: {}", root);
        print!("{}", storage.get_staged_entries());
        assert_eq!(root, "0d78b3".to_string());

        let entries = storage.get_staged_entries();
        let commit_hash = storage
            .commit(0, "Tezedge".to_string(), "persist changes".to_string())
            .unwrap();
        println!("\nCOMMIT time:0 author:'tezedge' message:'persist'");
        println!("ROOT: {}", get_short_hash(&commit_hash));
        if let Entry::Commit(c) = storage.get_entry(&commit_hash).unwrap() {
            println!("{} : Commit{{time:{}, message:{}, author:{}, root_hash:{}, parent_commit_hash: None}}", get_short_hash(&commit_hash), c.time,  c.message, c.author, get_short_hash(&c.root_hash));
        }
        print!("{}", entries);
        assert_eq!("e6de3f", get_short_hash(&commit_hash))
    }

    #[test]
    fn test_multiple_commit_hash() {
        let cache = Cache::new_lru_cache(32 * 1024 * 1024).unwrap();
        let mut storage = get_storage("ms_test_multiple_commit_hash", &cache);
        let _commit = storage.commit(0, "Tezos".to_string(), "Genesis".to_string());

        storage.set(
            &vec!["data".to_string(), "a".to_string(), "x".to_string()],
            &vec![97],
        );
        storage.copy(
            &vec!["data".to_string(), "a".to_string()],
            &vec!["data".to_string(), "b".to_string()],
        );
        storage.delete(&vec!["data".to_string(), "b".to_string(), "x".to_string()]);
        let commit = storage.commit(0, "Tezos".to_string(), "".to_string());

        assert_eq!([0x9B, 0xB0, 0x0D, 0x6E], commit.unwrap()[0..4]);
    }

    #[test]
    fn test_get() {
        let db_name = "ms_get_test";
        clean_db(db_name);

        let commit1;
        let commit2;
        let key_abc: &ContextKey = &vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let key_abx: &ContextKey = &vec!["a".to_string(), "b".to_string(), "x".to_string()];
        let key_eab: &ContextKey = &vec!["e".to_string(), "a".to_string(), "b".to_string()];
        let key_az: &ContextKey = &vec!["a".to_string(), "z".to_string()];
        let key_d: &ContextKey = &vec!["d".to_string()];

        {
            let cache = Cache::new_lru_cache(32 * 1024 * 1024).unwrap();
            let mut storage = get_storage(db_name, &cache);

            let res = storage.get(&vec![]);
            assert_eq!(res.unwrap().is_empty(), true);
            let res = storage.get(&vec!["a".to_string()]);
            assert_eq!(res.unwrap().is_empty(), true);

            storage.set(key_abc, &vec![1u8, 2u8]);
            storage.set(key_abx, &vec![3u8]);
            assert_eq!(storage.get(&key_abc).unwrap(), vec![1u8, 2u8]);
            assert_eq!(storage.get(&key_abx).unwrap(), vec![3u8]);
            commit1 = storage.commit(0, "".to_string(), "".to_string()).unwrap();

            storage.set(key_az, &vec![4u8]);
            storage.set(key_abx, &vec![5u8]);
            storage.set(key_d, &vec![6u8]);
            storage.set(key_eab, &vec![7u8]);
            assert_eq!(storage.get(key_az).unwrap(), vec![4u8]);
            assert_eq!(storage.get(key_abx).unwrap(), vec![5u8]);
            assert_eq!(storage.get(key_d).unwrap(), vec![6u8]);
            assert_eq!(storage.get(key_eab).unwrap(), vec![7u8]);
            commit2 = storage.commit(0, "".to_string(), "".to_string()).unwrap();
        }

        let cache = Cache::new_lru_cache(32 * 1024 * 1024).unwrap();
        let mut storage = get_storage(db_name, &cache);

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

    #[test]
    fn test_mem() {
        let db_name = "ms_test_mem";
        clean_db(db_name);

        let key_abc: &ContextKey = &vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let key_abx: &ContextKey = &vec!["a".to_string(), "b".to_string(), "x".to_string()];

        let cache = Cache::new_lru_cache(32 * 1024 * 1024).unwrap();
        let mut storage = get_storage(db_name, &cache);
        assert_eq!(storage.mem(&key_abc).unwrap(), false);
        assert_eq!(storage.mem(&key_abx).unwrap(), false);
        storage.set(key_abc, &vec![1u8, 2u8]);
        assert_eq!(storage.mem(&key_abc).unwrap(), true);
        assert_eq!(storage.mem(&key_abx).unwrap(), false);
        storage.set(key_abx, &vec![3u8]);
        assert_eq!(storage.mem(&key_abc).unwrap(), true);
        assert_eq!(storage.mem(&key_abx).unwrap(), true);
        storage.delete(key_abx);
        assert_eq!(storage.mem(&key_abc).unwrap(), true);
        assert_eq!(storage.mem(&key_abx).unwrap(), false);
    }

    #[test]
    fn test_delete_whole_tree1() {
        let db_name = "test_delete_whole_tree1";
        clean_db(db_name);

        let key_a: &ContextKey = &vec!["a".to_string()];

        let cache = Cache::new_lru_cache(32 * 1024 * 1024).unwrap();
        let mut storage = get_storage(db_name, &cache);

        storage.set(&vec!["a".to_string()], &vec![1u8, 2u8]);
        assert_eq!(storage.dirmem(&key_a).unwrap(), false);
        assert_eq!(storage.mem(&key_a).unwrap(), true);
        storage.delete(&vec!["a".to_string()]);
        assert_eq!(storage.dirmem(&key_a).unwrap(), false);
        assert_eq!(storage.mem(&key_a).unwrap(), false);
    }

    #[test]
    fn test_delete_whole_tree2() {
        let db_name = "test_delete_whole_tree2";
        clean_db(db_name);

        let key_abcd: &ContextKey = &vec![
            "a".to_string(),
            "b".to_string(),
            "c".to_string(),
            "d".to_string(),
        ];
        let key_abc: &ContextKey = &vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let key_ab: &ContextKey = &vec!["a".to_string(), "b".to_string()];
        let key_a: &ContextKey = &vec!["a".to_string()];

        let cache = Cache::new_lru_cache(32 * 1024 * 1024).unwrap();
        let mut storage = get_storage(db_name, &cache);

        storage.set(key_abcd, &vec![1u8, 2u8]);

        assert_eq!(storage.dirmem(&key_a).unwrap(), true);
        assert_eq!(storage.dirmem(&key_ab).unwrap(), true);
        assert_eq!(storage.dirmem(&key_abc).unwrap(), true);
        assert_eq!(storage.mem(&key_abcd).unwrap(), true);

        storage.delete(key_abcd);

        assert_eq!(storage.dirmem(&key_a).unwrap(), false);
        assert_eq!(storage.dirmem(&key_ab).unwrap(), false);
        assert_eq!(storage.dirmem(&key_abc).unwrap(), false);
        assert_eq!(storage.mem(&key_abcd).unwrap(), false);
    }

    #[test]
    fn test_delete_subtree() {
        let db_name = "test_delete_subtree";
        clean_db(db_name);

        let key_abcd: &ContextKey = &vec![
            "a".to_string(),
            "b".to_string(),
            "c".to_string(),
            "d".to_string(),
        ];
        let key_abc: &ContextKey = &vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let key_ab: &ContextKey = &vec!["a".to_string(), "b".to_string()];
        let key_a: &ContextKey = &vec!["a".to_string()];

        let key_ax: &ContextKey = &vec!["a".to_string(), "x".to_string()];

        let cache = Cache::new_lru_cache(32 * 1024 * 1024).unwrap();
        let mut storage = get_storage(db_name, &cache);

        storage.set(key_abcd, &vec![1u8, 2u8]);
        storage.set(key_ax, &vec![1u8, 2u8]);

        assert_eq!(storage.dirmem(&key_a).unwrap(), true);
        assert_eq!(storage.dirmem(&key_ab).unwrap(), true);
        assert_eq!(storage.dirmem(&key_abc).unwrap(), true);
        assert_eq!(storage.mem(&key_abcd).unwrap(), true);

        assert_eq!(storage.mem(&key_ax).unwrap(), true);

        storage.delete(key_abcd);

        assert_eq!(storage.dirmem(&key_a).unwrap(), true);
        assert_eq!(storage.mem(&key_ax).unwrap(), true);
        assert_eq!(storage.dirmem(&key_ab).unwrap(), false);
        assert_eq!(storage.dirmem(&key_abc).unwrap(), false);
        assert_eq!(storage.mem(&key_abcd).unwrap(), false);
    }

    #[test]
    fn test_copy() {
        let db_name = "ms_test_copy";
        clean_db(db_name);

        let cache = Cache::new_lru_cache(32 * 1024 * 1024).unwrap();
        let mut storage = get_storage(db_name, &cache);
        let key_abc: &ContextKey = &vec!["a".to_string(), "b".to_string(), "c".to_string()];
        storage.set(key_abc, &vec![1_u8]);
        storage.copy(&vec!["a".to_string()], &vec!["z".to_string()]);

        assert_eq!(
            vec![1_u8],
            storage
                .get(&vec!["z".to_string(), "b".to_string(), "c".to_string()])
                .unwrap()
        );
        // TODO test copy over commits
    }

    #[test]
    fn test_delete() {
        let db_name = "ms_test_delete";
        clean_db(db_name);

        let cache = Cache::new_lru_cache(32 * 1024 * 1024).unwrap();
        let mut storage = get_storage(db_name, &cache);
        let key_abc: &ContextKey = &vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let key_abx: &ContextKey = &vec!["a".to_string(), "b".to_string(), "x".to_string()];
        storage.set(key_abc, &vec![2_u8]);
        storage.set(key_abx, &vec![3_u8]);
        storage.delete(key_abx);
        let commit1 = storage.commit(0, "".to_string(), "".to_string()).unwrap();

        assert!(storage.get_history(&commit1, &key_abx).is_err());
    }

    #[test]
    fn test_deleted_entry_available() {
        let db_name = "ms_test_deleted_entry_available";
        clean_db(db_name);

        let cache = Cache::new_lru_cache(32 * 1024 * 1024).unwrap();
        let mut storage = get_storage(db_name, &cache);
        let key_abc: &ContextKey = &vec!["a".to_string(), "b".to_string(), "c".to_string()];
        storage.set(key_abc, &vec![2_u8]);
        let commit1 = storage.commit(0, "".to_string(), "".to_string()).unwrap();
        storage.delete(key_abc);
        let _commit2 = storage.commit(0, "".to_string(), "".to_string()).unwrap();

        assert_eq!(vec![2_u8], storage.get_history(&commit1, &key_abc).unwrap());
    }

    #[test]
    fn test_delete_in_separate_commit() {
        let db_name = "ms_test_delete_in_separate_commit";
        clean_db(db_name);

        let cache = Cache::new_lru_cache(32 * 1024 * 1024).unwrap();
        let mut storage = get_storage(db_name, &cache);
        let key_abc: &ContextKey = &vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let key_abx: &ContextKey = &vec!["a".to_string(), "b".to_string(), "x".to_string()];
        storage.set(key_abc, &vec![2_u8]).unwrap();
        storage.set(key_abx, &vec![3_u8]).unwrap();
        storage.commit(0, "".to_string(), "".to_string()).unwrap();

        storage.delete(key_abx);
        let commit2 = storage.commit(0, "".to_string(), "".to_string()).unwrap();

        assert!(storage.get_history(&commit2, &key_abx).is_err());
    }

    #[test]
    fn test_checkout() {
        let db_name = "ms_test_checkout";
        clean_db(db_name);

        let commit1;
        let commit2;
        let key_abc: &ContextKey = &vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let key_abx: &ContextKey = &vec!["a".to_string(), "b".to_string(), "x".to_string()];

        {
            let cache = Cache::new_lru_cache(32 * 1024 * 1024).unwrap();
            let mut storage = get_storage(db_name, &cache);
            storage.set(key_abc, &vec![1u8]).unwrap();
            storage.set(key_abx, &vec![2u8]).unwrap();
            commit1 = storage.commit(0, "".to_string(), "".to_string()).unwrap();

            storage.set(key_abc, &vec![3u8]).unwrap();
            storage.set(key_abx, &vec![4u8]).unwrap();
            commit2 = storage.commit(0, "".to_string(), "".to_string()).unwrap();
        }

        let cache = Cache::new_lru_cache(32 * 1024 * 1024).unwrap();
        let mut storage = get_storage(db_name, &cache);
        storage.checkout(&commit1);
        assert_eq!(storage.get(&key_abc).unwrap(), vec![1u8]);
        assert_eq!(storage.get(&key_abx).unwrap(), vec![2u8]);
        // this set be wiped by checkout
        storage.set(key_abc, &vec![8u8]).unwrap();

        storage.checkout(&commit2);
        assert_eq!(storage.get(&key_abc).unwrap(), vec![3u8]);
        assert_eq!(storage.get(&key_abx).unwrap(), vec![4u8]);
    }

    #[test]
    fn test_persistence_over_reopens() {
        let db_name = "ms_test_persistence_over_reopens";
        {
            clean_db(db_name);
        }

        let key_abc: &ContextKey = &vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let commit1;
        {
            let cache = Cache::new_lru_cache(32 * 1024 * 1024).unwrap();
            let mut storage = get_storage(db_name, &cache);
            let key_abx: &ContextKey = &vec!["a".to_string(), "b".to_string(), "x".to_string()];
            storage.set(key_abc, &vec![2_u8]).unwrap();
            storage.set(key_abx, &vec![3_u8]).unwrap();
            commit1 = storage.commit(0, "".to_string(), "".to_string()).unwrap();
        }

        let cache = Cache::new_lru_cache(32 * 1024 * 1024).unwrap();
        let mut storage = get_storage(db_name, &cache);
        assert_eq!(vec![2_u8], storage.get_history(&commit1, &key_abc).unwrap());
    }

    // Test a DB error by writing into a read-only database.
    #[test]
    fn test_db_error() {
        let db_name = "ms_test_db_error";
        {
            clean_db(db_name);
            let cache = Cache::new_lru_cache(32 * 1024 * 1024).unwrap();
            get_storage(db_name, &cache);
        }

        let db = DB::open_for_read_only(&Options::default(), get_db_name(db_name), true).unwrap();
        let mut storage = MerkleStorage::new(Arc::new(db));
        storage.set(&vec!["a".to_string()], &vec![1u8]);
        let res = storage.commit(0, "".to_string(), "".to_string());

        assert!(matches!(res.err().unwrap(), MerkleError::DBError { .. }));
    }

    // Test getting entire tree in string format for JSON RPC
    #[test]
    fn test_get_context_tree_by_prefix() {
        let db_name = "ms_test_get_context_tree_by_prefix";
        {
            clean_db(db_name);
        }

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

        let cache = Cache::new_lru_cache(32 * 1024 * 1024).unwrap();
        let mut storage = get_storage(db_name, &cache);
        let _commit = storage.commit(0, "Tezos".to_string(), "Genesis".to_string());

        storage.set(
            &vec!["data".to_string(), "a".to_string(), "x".to_string()],
            &vec![3, 4],
        );
        storage.set(&vec!["data".to_string(), "a".to_string()], &vec![1, 2]);
        storage.set(
            &vec![
                "data".to_string(),
                "a".to_string(),
                "x".to_string(),
                "y".to_string(),
            ],
            &vec![5, 6],
        );
        storage.set(
            &vec![
                "data".to_string(),
                "b".to_string(),
                "x".to_string(),
                "y".to_string(),
            ],
            &vec![7, 8],
        );
        storage.set(&vec!["data".to_string(), "c".to_string()], &vec![1, 2]);
        storage.set(
            &vec![
                "adata".to_string(),
                "b".to_string(),
                "x".to_string(),
                "y".to_string(),
            ],
            &vec![9, 10],
        );
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

    fn test_backtracking_on_set() {
        let db_name = "test_delete_whole_tree2";
        clean_db(db_name);

        let key_abcd: &ContextKey = &vec![
            "a".to_string(),
            "b".to_string(),
            "c".to_string(),
            "d".to_string(),
        ];
        let key_abc: &ContextKey = &vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let key_ab: &ContextKey = &vec!["a".to_string(), "b".to_string()];
        let key_a: &ContextKey = &vec!["a".to_string()];

        let cache = Cache::new_lru_cache(32 * 1024 * 1024).unwrap();
        let mut storage = get_storage(db_name, &cache);

        storage.set(key_abcd, &vec![1u8]);

        storage.set(key_abcd, &vec![2u8]);

        assert_eq!(storage.dirmem(&key_a).unwrap(), true);
        assert_eq!(storage.dirmem(&key_ab).unwrap(), true);
        assert_eq!(storage.dirmem(&key_abc).unwrap(), true);
        assert_eq!(storage.mem(&key_abcd).unwrap(), true);

        storage.delete(key_abcd);

        assert_eq!(storage.dirmem(&key_a).unwrap(), false);
        assert_eq!(storage.dirmem(&key_ab).unwrap(), false);
        assert_eq!(storage.dirmem(&key_abc).unwrap(), false);
        assert_eq!(storage.mem(&key_abcd).unwrap(), false);
    }
}
