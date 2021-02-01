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
use std::collections::hash_map::Entry as MapEntry;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::convert::TryInto;
use std::hash::Hash;
use std::time::Instant;
use std::sync::Arc;
use std::iter::FromIterator;
use hex;

use blake2::digest::{Update, VariableOutput};
use blake2::VarBlake2b;
use failure::{Fail, Error};
use serde::Deserialize;
use serde::Serialize;
use rocksdb::{ColumnFamilyDescriptor, Cache};

use crypto::hash::HashType;

use crate::context_action_storage::{ContextAction, ContextActionStorage};
use crate::storage_backend::{StorageBackend, StorageBackendStats, StorageBackendError};
use crate::persistent::{BincodeEncoded, KeyValueSchema, default_table_options};

const HASH_LEN: usize = 32;

pub type ContextKey = Vec<String>;
pub type ContextValue = Vec<u8>;
pub type EntryHash = [u8; HASH_LEN];

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum NodeKind {
    NonLeaf,
    Leaf,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub node_kind: NodeKind,
    pub entry_hash: EntryHash,
}

// Tree must be an ordered structure for consistent hash in hash_tree
// Currently immutable OrdMap is used to allow cloning trees without too much overhead
pub type Tree = BTreeMap<String, Node>;

#[derive(Debug, Hash, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Commit {
    pub parent_commit_hash: Option<EntryHash>,
    pub root_hash: EntryHash,
    pub time: u64,
    pub author: String,
    pub message: String,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum Entry {
    Tree(Tree),
    Blob(ContextValue),
    Commit(Commit),
}

impl Entry {
    pub fn get_type(&self) -> &'static str {
        match self {
            Entry::Blob(_) => "blob",
            Entry::Tree(_) => "tree",
            Entry::Commit(_) => "commit",
        }
    }
}

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

pub type MerkleStorageKV = dyn StorageBackend + Sync + Send;

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

pub type RefCnt = usize;

pub struct MerkleStorage {
    /// tree with current staging area (currently checked out context)
    current_stage_tree: Option<Tree>,
    current_stage_tree_hash: Option<EntryHash>,
    db: Box<MerkleStorageKV>,
    /// all entries in current staging area
    staged: Vec<(EntryHash, RefCnt, Entry)>,
    /// HashMap for looking up entry index in self.staged by hash
    staged_indices: HashMap<EntryHash, usize>,
    last_commit_hash: Option<EntryHash>,
    /// storage latency statistics
    perf_stats: MerklePerfStats,
    block_latencies: BlockLatencies,
    /// list of all actions done on staging area
    actions: Arc<Vec<Action>>,
}

#[derive(Debug, Fail)]
pub enum MerkleError {
    /// External libs errors
    #[fail(display = "KVStore error: {:?}", error)]
    DBError { error: StorageBackendError },
    #[fail(display = "Serialization error: {:?}", error)]
    SerializationError { error: bincode::Error },
    #[fail(display = "Backend error: {:?}", error)]
    StorageBackendError {
        error: StorageBackendError,
    },

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

impl From<StorageBackendError> for MerkleError {
    fn from(error: StorageBackendError) -> Self {
        MerkleError::StorageBackendError { error }
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

/// Block application latencies
#[derive(Serialize, Default, Debug, Clone)]
pub struct BlockLatencies {
    latencies: Vec<u64>,
    current: u64,
}

impl BlockLatencies {
    fn new() -> Self {
        Default::default()
    }

    fn update(&mut self, latency: u64) {
        self.current += latency;
    }

    fn end_block(&mut self) {
        self.latencies.push(self.current);
        self.current = 0;
    }

    pub fn get(&self, offset_from_last_applied: usize) -> Option<u64> {
        self.latencies.len()
            .checked_sub(offset_from_last_applied + 1)
            .and_then(|index| self.latencies.get(index))
            .map(|x| *x)
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
    pub perf_stats: MerklePerfStats,
    pub kv_store_stats: StorageBackendStats,
}

impl BincodeEncoded for EntryHash {}

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
pub fn hash_tree(tree: &Tree) -> Result<EntryHash, MerkleError> {
    let mut hasher = VarBlake2b::new(HASH_LEN).unwrap();

    hasher.update(&(tree.len() as u64).to_be_bytes());
    tree.iter().for_each(|(k, v)| {
        hasher.update(&encode_irmin_node_kind(&v.node_kind));
        hasher.update(&[k.len() as u8]);
        hasher.update(&k.clone().into_bytes());
        hasher.update(&(HASH_LEN as u64).to_be_bytes());
        hasher.update(&v.entry_hash);
    });

    Ok(hasher.finalize_boxed().as_ref().try_into()?)
}

// Calculates hash of BLOB
// uses BLAKE2 binary 256 length hash function
// hash is calculated as <length of data (8 bytes)><data>
pub fn hash_blob(blob: &ContextValue) -> Result<EntryHash, MerkleError> {
    let mut hasher = VarBlake2b::new(HASH_LEN).unwrap();
    hasher.update(&(blob.len() as u64).to_be_bytes());
    hasher.update(blob);

    Ok(hasher.finalize_boxed().as_ref().try_into()?)
}

// Calculates hash of commit
// uses BLAKE2 binary 256 length hash function
// hash is calculated as:
// <hash length (8 bytes)><tree hash bytes>
// <length of parent hash (8bytes)><parent hash bytes>
// <time in epoch format (8bytes)
// <commit author name length (8bytes)><commit author name bytes>
// <commit message length (8bytes)><commit message bytes>
pub fn hash_commit(commit: &Commit) -> Result<EntryHash, MerkleError> {
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

    Ok(hasher.finalize_boxed().as_ref().try_into()?)
}

pub fn hash_entry(entry: &Entry) -> Result<EntryHash, MerkleError> {
    match entry {
        Entry::Commit(commit) => hash_commit(&commit),
        Entry::Tree(tree) => hash_tree(&tree),
        Entry::Blob(blob) => hash_blob(blob),
    }
}

#[derive(Debug, Fail)]
pub enum CheckEntryHashError {
    #[fail(display = "MerkleError error: {:?}", error)]
    MerkleError { error: MerkleError },
    #[fail(display = "Calculated hash for {} not matching expected hash: expected {:?}, calculated {:?}", entry_type, calculated, expected)]
    InvalidHashError { entry_type: String, calculated: String, expected: String }
}

impl From<MerkleError> for CheckEntryHashError {
    fn from(error: MerkleError) -> Self { CheckEntryHashError::MerkleError { error } }
}

/// Recursively check if entry hash is valid
pub fn check_commit_hashes<I>(storage: &MerkleStorage, commits: I) -> Result<(), CheckEntryHashError>
where I: IntoIterator<Item = EntryHash> {
    let mut entries = Vec::from_iter(commits);
    let mut checked = HashSet::new();

    loop {
        let entry_hash = match entries.pop() {
            Some(hash) => hash,
            None => break,
        };
        if checked.contains(&entry_hash) {
            continue;
        }

        let entry = storage.get_entry(&entry_hash)?;
        let entry_type = entry.get_type();

        let calculated_hash = match entry {
            Entry::Blob(blob) => { hash_blob(&blob)? }
            Entry::Tree(tree) => {
                let hash = hash_tree(&tree)?;
                for (_, node) in tree.into_iter() {
                    entries.push(node.entry_hash);
                }
                hash
            }
            Entry::Commit(commit) => {
                let hash = hash_commit(&commit)?;
                entries.push(commit.root_hash);
                hash
            }
        };

        if &calculated_hash != &entry_hash {
            return Err(CheckEntryHashError::InvalidHashError {
                entry_type: entry_type.to_string(),
                expected: HashType::ContextHash.hash_to_b58check(&entry_hash),
                calculated: HashType::ContextHash.hash_to_b58check(&calculated_hash),
            });
        }

        checked.insert(entry_hash);
    }

    Ok(())
}

impl MerkleStorage {
    pub fn new(db: Box<MerkleStorageKV>) -> Self {
        MerkleStorage {
            db,
            staged: Vec::new(),
            staged_indices: HashMap::new(),
            current_stage_tree: None,
            current_stage_tree_hash: None,
            last_commit_hash: None,
            perf_stats: MerklePerfStats {
                global: HashMap::new(),
                perpath: HashMap::new(),
            },
            block_latencies: Default::default(),
            actions: Arc::new(Vec::new()),
        }
    }

    pub fn is_persisted(&self) -> bool {
        self.db.is_persisted()
    }

    /// if `MerkleStorage` is not persisted, restore it from `ContextActionStorage`.
    /// TODO: modify general `Error` with `MerkleError`
    pub fn apply_context_actions_from_store(&mut self, ctx_action_storage: &ContextActionStorage) -> Result<(), Error> {
        let it = ctx_action_storage.get_by_mut_action_types()?
            .into_iter()
            .map(|x| x.action);
        self.apply_context_actions(it)?;
        Ok(())
    }

    pub fn apply_context_actions<I>(&mut self, it: I) -> Result<(), MerkleError>
    where I: IntoIterator<Item = ContextAction>,
    {
        for context_action in it {
            self.apply_context_action(&context_action)?;
        }
        Ok(())
    }

    pub fn apply_context_action(&mut self, context_action: &ContextAction) -> Result<(), MerkleError> {
        match context_action {
            ContextAction::Set { key, value, .. } => {
                self.set(&key, &value)?;
            }
            ContextAction::Copy { to_key, from_key, .. } => {
                self.copy(&from_key, &to_key)?;
            }
            ContextAction::Delete { key, .. } => {
                self.delete(&key)?;
            }
            ContextAction::RemoveRecursively { key, .. } => {
                self.delete(&key)?;
            }
            ContextAction::Commit { author, message, date, new_context_hash, .. } => {
                let commit_hash = self.commit(*date as u64, author.to_string(), message.to_string())?;
                assert_eq!(
                    &commit_hash,
                    &new_context_hash[..],
                    "Invalid commit_hash detected while applying context action to MerkleStorage, expected: {}, but was: {}",
                    HashType::ContextHash.hash_to_b58check(&new_context_hash),
                    HashType::ContextHash.hash_to_b58check(&commit_hash),
                );
            }
            ContextAction::Checkout { context_hash, .. } => {
                self.checkout(context_hash.as_slice().try_into()?)?;
            }
            // ContextAction::Get { key, value, .. } => {
            //     assert_eq!(&self.get(key)?, value);
            // }
            // ContextAction::Mem { key, value, .. } => {
            //     assert_eq!(&self.mem(key)?, value);
            // }
            // ContextAction::DirMem { key, value, .. } => {
            //     assert_eq!(&self.dirmem(key)?, value);
            // }
            _ => {}
        };
        Ok(())
    }

    /// Get value from current staged root
    pub fn get(&mut self, key: &ContextKey) -> Result<ContextValue, MerkleError> {
        let instant = Instant::now();
        // build staging tree from saved list of actions (set/copy/delete)
        // note: this can be slow if there are a lot of actions
        self.apply_actions_to_staging_area()?;

        let root = &self.get_staged_root()?;
        let root_hash = hash_tree(&root)?;

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
        self.apply_actions_to_staging_area()?;

        let root = &self.get_staged_root()?;
        let root_hash = hash_tree(&root)?;

        let rv = self.value_exists(&root_hash, key);
        self.update_execution_stats("Mem".to_string(), Some(&key), &instant);
        rv
    }

    /// Check if directory exists in current staged root
    pub fn dirmem(&mut self, key: &ContextKey) -> Result<bool, MerkleError> {
        let instant = Instant::now();
        // build staging tree from saved list of actions (set/copy/delete)
        // note: this can be slow if there are a lot of actions
        self.apply_actions_to_staging_area()?;

        let root = &self.get_staged_root()?;
        let root_hash = hash_tree(&root)?;

        let rv = self.directory_exists(&root_hash, key);
        self.update_execution_stats("DirMem".to_string(), Some(&key), &instant);
        rv
    }

    /// Get value. Staging area is checked first, then last (checked out) commit.
    pub fn get_by_prefix(
        &mut self,
        prefix: &ContextKey,
    ) -> Result<Option<Vec<(ContextKey, ContextValue)>>, MerkleError> {
        let root = self.get_staged_root()?;
        self._get_key_values_by_prefix(root, prefix)
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
        let instant = Instant::now();
        let commit = self.get_commit(&context_hash)?;
        self.current_stage_tree = Some(self.get_tree(&commit.root_hash)?);
        self.current_stage_tree_hash = Some(commit.root_hash);
        self.last_commit_hash = Some(*context_hash);
        self.staged = Vec::new();
        self.staged_indices = HashMap::new();
        // clear list of actions
        self.actions = Arc::new(Vec::new());
        self.update_execution_stats("Checkout".to_string(), None, &instant);
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
        let instant = Instant::now();

        // build staging tree from saved list of actions (set/copy/delete)
        self.apply_actions_to_staging_area()?;

        let staged_root = self.get_staged_root()?;
        let staged_root_hash = hash_tree(&staged_root)?;
        let parent_commit_hash = self.last_commit_hash;

        let new_commit = Commit {
            root_hash: staged_root_hash,
            parent_commit_hash,
            time,
            author,
            message,
        };
        let entry = Entry::Commit(new_commit.clone());

        let new_commit_hash = hash_commit(&new_commit)?;
        self.put_to_staging_area(&new_commit_hash, entry.clone())?;
        self.persist_staged_entry_to_db(&entry)?;
        self.staged = Vec::new();
        self.staged_indices = HashMap::new();
        self.last_commit_hash = Some(new_commit_hash.clone());

        self.update_execution_stats("Commit".to_string(), None, &instant);
        Ok(new_commit_hash)
    }

    pub fn start_new_cycle(&mut self) -> Result<(), MerkleError> {
        let instant = Instant::now();
        self.db.start_new_cycle(self.last_commit_hash.clone());
        self.update_execution_stats("GC".to_string(), None, &instant);
        Ok(())
    }

    /// Set key/val to the staging area.
    pub fn set(&mut self, key: &ContextKey, value: &ContextValue) -> Result<(), MerkleError> {
        let instant = Instant::now();
        let act = Arc::make_mut(&mut self.actions);
        // store action
        act.push(Action::Set(SetAction {
            key: key.to_vec(),
            value: value.to_vec(),
        }));
        self.update_execution_stats("Set".to_string(), Some(&key), &instant);
        Ok(())
    }

    /// Delete an item from the staging area.
    pub fn delete(&mut self, key: &ContextKey) -> Result<(), MerkleError> {
        let instant = Instant::now();
        let act = Arc::make_mut(&mut self.actions);
        // store action
        act.push(Action::Remove(RemoveAction { key: key.to_vec() }));
        self.update_execution_stats("Delete".to_string(), Some(&key), &instant);
        Ok(())
    }

    /// Copy subtree under a new path.
    /// TODO Consider copying values!
    pub fn copy(&mut self, from_key: &ContextKey, to_key: &ContextKey) -> Result<(), MerkleError> {
        let instant = Instant::now();
        let act = Arc::make_mut(&mut self.actions);
        // store action
        act.push(Action::Copy(CopyAction {
            from_key: from_key.to_vec(),
            to_key: to_key.to_vec(),
        }));
        // TODO: do we need to include from_key in stats?
        self.update_execution_stats("CopyToDiff".to_string(), Some(&to_key), &instant);
        Ok(())
    }

    fn add_empty_tree_to_staging(&mut self) -> Result<Option<usize>, MerkleError> {
        let tree = Tree::new();
        let hash = hash_tree(&tree)?;
        self.put_to_staging_area(&hash, Entry::Tree(tree))
    }

    /// If current staging tree does not exist yet, create a new empty tree
    fn ensure_stage_tree_exists(&mut self) -> Result<(), MerkleError> {
        match &self.current_stage_tree {
            None => {
                let tree = Tree::new();
                self.current_stage_tree = Some(tree.clone());
                let hash = hash_tree(&tree)?;
                self.current_stage_tree_hash = Some(hash);
                self.put_to_staging_area(&hash, Entry::Tree(tree))?;
            }
            Some(_tree) => (),
        }
        Ok(())
    }

    fn increase_refcnt_for_staging_entry(&mut self, hash: &EntryHash) -> Result<(), MerkleError> {
        let idx = self.staged_get_idx(hash);
        match idx {
            Some(idx) => {
                self.staged[idx].1 += 1;
            }
            None => {
                return Err(MerkleError::EntryNotFoundInStaging {
                    hash: HashType::ContextHash.hash_to_b58check(hash),
                });
            }
        }
        Ok(())
    }

    /// Walk through actions list and apply actions sequentially.
    /// All new blobs/trees and their hashes will be added to self.staged HashMap.
    /// Current root tree of staging area is always in self.current_stage_tree.
    /// This function must be called before commit() in order to prepare staging area for
    /// committing and persisting to the database.
    fn apply_actions_to_staging_area(&mut self) -> Result<(), MerkleError> {
        // if there is no staging tree yet, create an empty one and add it
        self.ensure_stage_tree_exists()?;

        // clone reference to actions (it is an Arc<> clone)
        let actions = self.actions.clone();
        for action in actions.iter() {
            match action {
                Action::Set(set) => {
                    let root_hash = self.current_stage_tree_hash.unwrap();
                    let key = &set.key;
                    let blob_hash = hash_blob(&set.value)?;
                    self.put_to_staging_area(&blob_hash, Entry::Blob(set.value.clone()))?;
                    let new_node = Node {
                        entry_hash: blob_hash,
                        node_kind: NodeKind::Leaf,
                    };

                    //TODO inefficient - maybe instead of pushing root tree here just don't remove this entry on commit() (where we set self.staged to Vec::new())
                    self.put_to_staging_area(&root_hash, self.get_entry(&root_hash)?)?;
                    let new_hash =
                        self.compute_new_root_with_change(&root_hash, &key, Some(new_node))?;

                    // Put the newly created Tree into current_staging_tree
                    // TODO: can be optimized (unfortunately get_tree() currently clones tree)
                    // e.g. maybe make current_stage_tree an index into self.staged
                    self.current_stage_tree = Some(self.get_tree(&new_hash)?);
                    self.current_stage_tree_hash = Some(new_hash);
                }

                Action::Copy(copy) => {
                    let root_hash = self.current_stage_tree_hash.unwrap();
                    let root = self.get_entry(&root_hash)?;
                    //TODO inefficient - maybe instead of pushing root tree here just don't remove this entry on commit() (where we set self.staged to Vec::new())
                    self.put_to_staging_area(&root_hash, self.get_entry(&root_hash)?)?;
                    let new_hash;
                    if let Entry::Tree(root) = root {
                        //TODO: assert that source_tree isn't Tree::new() ?
                        let source_tree = self.find_tree(&root, &copy.from_key)?;
                        let source_tree_hash = hash_tree(&source_tree)?;
                        new_hash = self.compute_new_root_with_change(
                            &root_hash,
                            &copy.to_key,
                            Some(self.get_non_leaf(source_tree_hash)),
                        )?;
                    //TODO: check if there is need to increment refcounts recursively
                    } else {
                        return Err(MerkleError::FoundUnexpectedStructure {
                            sought: "Tree".to_string(),
                            found: "Blob/Commit".to_string(),
                        });
                    }
                    self.current_stage_tree = Some(self.get_tree(&new_hash)?);
                    self.current_stage_tree_hash = Some(new_hash);
                }

                Action::Remove(remove) => {
                    let root_hash = self.current_stage_tree_hash.unwrap();
                    //TODO inefficient - maybe instead of pushing root tree here just don't remove this entry on commit() (where we set self.staged to Vec::new())
                    self.put_to_staging_area(&root_hash, self.get_entry(&root_hash)?)?;
                    let new_hash =
                        self.compute_new_root_with_change(&root_hash, &remove.key, None)?;
                    //TODO: check if there is need to decrement refcounts recursively
                    self.current_stage_tree = Some(self.get_tree(&new_hash)?);
                    self.current_stage_tree_hash = Some(new_hash);
                }
            }
        }

        // clear list of actions
        self.actions = Arc::new(Vec::new());

        Ok(())
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
    fn compute_new_root_with_change(
        &mut self,
        root_hash: &EntryHash,
        key: &[String],
        new_node: Option<Node>,
    ) -> Result<EntryHash, MerkleError> {
        assert_eq!(key.is_empty(), false);
        if key.is_empty() {
            match new_node {
                Some(n) => {
                    return Ok(n.entry_hash);
                }
                None => {
                    return Ok(*root_hash);
                }
            }
        }

        // root tree is always in staging area
        let root_idx = self.staged_get_idx(&root_hash).unwrap();

        let last = key.last().unwrap();
        let path = &key[..key.len() - 1];

        // find tree by path and get new copy of it
        let mut idx = self.find_tree_staging(root_idx, path)?;
        let mut empty_tree_existed = false;
        if idx.is_none() {
            // node doesn't exist or is Blob, create empty tree unless it is staged already
            let empty_tree = Tree::new();
            let idx_of_empty_tree = self.staged_get_idx(&hash_tree(&empty_tree)?);
            if idx_of_empty_tree.is_some() {
                idx = idx_of_empty_tree;
                empty_tree_existed = true;
            } else {
                idx = self.add_empty_tree_to_staging()?;
            }
        }

        // make the modification of tree in place (in self.staged entry) if possible, otherwise
        // copy tree to a new entry in self.staged and then modify it
        match idx {
            Some(idx) => {
                let mut idx = idx;
                // first check if we can modify tree in place
                let mut in_place = true;
                let refcnt = self.staged[idx].1;
                if refcnt > 1 || empty_tree_existed {
                    // can't modify in place as it's used elsewhere, must copy tree to a new entry
                    in_place = false;
                    let len = self.staged.len();
                    self.staged.push(self.staged[idx].clone());
                    // set refcnt of new entry to 1
                    self.staged[len].1 = 1;
                    idx = len;
                }

                let (ref mut tree_hash, _, ref mut tree_ref) = self.staged[idx];
                if let Entry::Tree(tree) = tree_ref {
                    // make the modification of tree at key
                    match new_node {
                        //TODO: decrement refcnt here - but test for all edge cases first
                        None => (tree).remove(last),
                        Some(new_node) => tree.insert(last.clone(), new_node),
                    };
                    // calculate hash of modified tree
                    let new_tree_hash = hash_tree(&tree)?;
                    let old_hash = *tree_hash;

                    // If tree was modified in place, remove old hash from staged_indices as it no longer exists
                    if in_place {
                        self.staged_indices.remove(&old_hash);
                    }

                    // Entry was modified, so its hash must be updated
                    // note: old tree is gone, will need to be recreated for backtracking
                    *tree_hash = new_tree_hash;

                    let tree_is_empty = tree.is_empty();

                    match self.staged_indices.entry(new_tree_hash) {
                        MapEntry::Occupied(_) => {
                            // entry already exists in staging, increase its refcnt only
                            // staged_indices will point to the other entry while this entry will be
                            // unreachable (wasting space, but removing it would require changing all
                            // other indices)
                            self.increase_refcnt_for_staging_entry(&new_tree_hash)?;
                        }
                        MapEntry::Vacant(_) => {
                            self.staged_indices.insert(new_tree_hash, idx);
                        }
                    }

                    if tree_is_empty {
                        // last element was removed, delete this node
                        if path.is_empty() {
                            // tree was removed completely - the entire staging tree up to the root
                            return Ok(new_tree_hash);
                        }
                        self.compute_new_root_with_change(&root_hash, path, None)
                    } else {
                        if path.is_empty() {
                            return Ok(new_tree_hash);
                        }
                        self.compute_new_root_with_change(
                            &root_hash,
                            path,
                            Some(self.get_non_leaf(new_tree_hash)),
                        )
                    }
                } else {
                    // compute_new_root_with_change: Entry is not a Tree
                    Err(MerkleError::FoundUnexpectedStructure {
                        sought: "Tree".to_string(),
                        found: "Blob/Commit".to_string(),
                    })
                }
            }
            None => {
                // error getting tree from staged - should not happen
                panic!("compute_new_root_with_change: idx is None");
            }
        }
    }

    // returns index to self.staged with found subtree and its hash
    fn find_tree_staging(
        &mut self,
        root_idx: usize,
        key: &[String],
    ) -> Result<Option<usize>, MerkleError> {
        if key.is_empty() {
            return Ok(Some(root_idx));
        }

        let (_, _, ref root) = self.staged[root_idx];
        let child_node = match root {
            Entry::Tree(root) => match root.get(key.first().unwrap()) {
                Some(node) => node,
                None => {
                    return Ok(None);
                }
            },
            _ => {
                return Ok(None); //TODO: panic?
            }
        };

        let entry_hash = child_node.entry_hash;
        let entry_idx = match self.staged_get_idx(&entry_hash) {
            Some(idx) => idx,
            None => {
                // not in staging, get Entry from database and put in staging
                let last_idx =
                    self.put_to_staging_area(&entry_hash, self.get_entry_db(&entry_hash)?);
                last_idx.unwrap().unwrap()
            }
        };

        let (_, _, ref entry) = self.staged[entry_idx];
        match entry {
            Entry::Tree(_) => {
                if key.len() == 1 {
                    // return the found tree
                    Ok(Some(entry_idx))
                } else {
                    self.find_tree_staging(entry_idx, &key[1..])
                }
            }
            Entry::Blob(_) => Ok(None),
            Entry::Commit { .. } => Err(MerkleError::FoundUnexpectedStructure {
                sought: "Tree/Blob".to_string(),
                found: "commit".to_string(),
            }),
        }
    }

    /// get Entry by hash
    fn staged_get(&self, hash: &EntryHash) -> Option<&Entry> {
        // lookup index by hash
        match self.staged_get_idx(hash) {
            Some(idx) => Some(&self.staged[idx].2),
            None => None,
        }
    }

    /// get index to self.staged containing entry by hash
    fn staged_get_idx(&self, hash: &EntryHash) -> Option<usize> {
        match self.staged_indices.get(hash) {
            Some(idx) => {
                // ensure staged_indices and staged are synchronized
                assert_eq!(self.staged[*idx].0, *hash);
                Some(*idx)
            }
            None => None,
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
    fn get_staged_root(&mut self) -> Result<Tree, MerkleError> {
        match &self.current_stage_tree {
            None => {
                let tree = Tree::new();
                self.put_to_staging_area(&hash_tree(&tree)?, Entry::Tree(tree.clone()))?;
                Ok(tree)
            }
            Some(tree) => Ok(tree.clone()),
        }
    }

    /// Put entry in staging area
    /// Note: if entry already exists, its reference count is incremented
    fn put_to_staging_area(
        &mut self,
        key: &EntryHash,
        value: Entry,
    ) -> Result<Option<usize>, MerkleError> {
        if let Some(idx) = self.staged_get_idx(key) {
            // entry already exists - increase its reference count only
            self.increase_refcnt_for_staging_entry(key)?;
            return Ok(Some(idx));
        }

        // new entry
        let idx = self.staged.len();
        // add entry and set its reference count to 1
        self.staged.push((*key, 1, value));
        self.staged_indices.insert(*key, idx);
        Ok(Some(idx))
    }

    fn persist_staged_entry_to_db(&mut self, entry: &Entry) -> Result<(), MerkleError> {
        let entry_hash = hash_entry(entry)?;
        self.db.put(
            entry_hash.clone(),
            bincode::serialize(entry)?,
        )?;

        match entry {
            Entry::Blob(_) => Ok(()),
            Entry::Tree(tree) => {
                // Go through all descendants and gather errors. Remap error if there is a failure
                // anywhere in the recursion paths. TODO: is revert possible?
                for (_, child_node) in tree.iter() {
                    let node_entry = self.staged_get(&child_node.entry_hash).cloned();
                    match node_entry {
                        // if child node isn't in staging area it means it
                        // hasn't changed from last commit and we should reuse it.
                        None => self.db.mark_reused(child_node.entry_hash),
                        Some(entry) => self.persist_staged_entry_to_db(&entry)?,
                    }
                }
                Ok(())
            }
            Entry::Commit(commit) => {
                match self.get_entry(&commit.root_hash) {
                    Err(err) => Err(err),
                    Ok(entry) => self.persist_staged_entry_to_db(&entry),
                }
            }
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

    fn get_entry_db(&self, hash: &EntryHash) -> Result<Entry, MerkleError> {
        let entry_bytes = self.db.get(hash)?;
        match entry_bytes {
            None => Err(MerkleError::EntryNotFound {
                hash: HashType::ContextHash.hash_to_b58check(hash),
            }),
            Some(entry_bytes) => Ok(bincode::deserialize(&entry_bytes)?),
        }
    }
    /// Get entry from staging area or look up in DB if not found
    fn get_entry(&self, hash: &EntryHash) -> Result<Entry, MerkleError> {
        match self.staged_get(hash) {
            None => Ok(self.get_entry_db(hash)?),
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
        for (hash, _, entry) in &self.staged {
            match entry {
                Entry::Blob(blob) => {
                    result += &format!("{}: Value {:?}, \n", hex::encode(&hash[0..3]), blob);
                }

                Entry::Tree(tree) => {
                    if tree.is_empty() {
                        continue;
                    }
                    let tree_hash = &hash_tree(tree).unwrap()[0..3];
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
            perf_stats: perf,
            kv_store_stats: self.db.get_total_stats(),
        })
    }

    pub fn get_block_latency(&self, offset_from_last_applied: usize) -> Option<u64> {
        self.block_latencies.get(offset_from_last_applied)
    }

    /// Update global and per-path execution stats. Pass Instant with operation execution time
    pub fn update_execution_stats(
        &mut self,
        op: String,
        path: Option<&ContextKey>,
        instant: &Instant,
    ) {
        // stop timer and get duration
        let exec_time_nanos = instant.elapsed().as_nanos();
        let exec_time: f64 = exec_time_nanos as f64;

        self.block_latencies.update(exec_time_nanos as u64);
        // commit signifies end of the block
        if &op == "Commit" {
            self.block_latencies.end_block();
        }

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

    /// Blocks until gc finishes. Useful for testing.
    pub fn wait_for_gc_finish(&self) {
        self.db.wait_for_gc_finish();
    }
}

#[cfg(test)]
#[allow(unused_must_use)]
mod tests {
    use assert_json_diff::assert_json_eq;
    use std::time::Duration;
    use std::sync::RwLock;
    use std::ops::Deref;
    use std::path::{Path, PathBuf};
    use rocksdb::{DB, Cache, Options};
    use std::{fs, env};

    use crate::context_key;
    use super::*;
    use crate::backend::{RocksDBBackend, InMemoryBackend, SledBackend};

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

    fn get_db(db_name: &str, cache: &Cache) -> RocksDBBackend {
        let rocks = open_db(get_db_name(db_name), &cache);
        RocksDBBackend::new(Arc::new(rocks), MerkleStorage::name())
    }

    fn get_storage(dn_name: &str, cache: &Cache) -> MerkleStorage {
        MerkleStorage::new(Box::new(get_db(dn_name, &cache)))
    }

    fn get_mem_storage(db : Arc<RwLock<HashMap<EntryHash, ContextValue>>>) -> MerkleStorage {
        MerkleStorage::new(Box::new(InMemoryBackend::new(db)))
    }

    fn get_sled_storage(db : sled::Tree) -> MerkleStorage {
        MerkleStorage::new(Box::new(SledBackend::new(db)))
    }

    fn clean_db(db_name: &str) {
        let _ = DB::destroy(&Options::default(), get_db_name(db_name));
        let _ = fs::remove_dir_all(get_db_name(db_name));
    }

    fn empty_mem_storage() -> MerkleStorage {
        get_mem_storage(Arc::new(RwLock::new(HashMap::new())))
    }

    #[test]
    fn test_duplicate_entry_in_staging() {
        let mut storage = empty_mem_storage();
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
        assert_eq!(hex::encode(hash_blob(&blob).unwrap()), calcualted_hash);
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

        assert_eq!(
            calculated_commit_hash.as_ref(),
            hash_commit(&dummy_commit).unwrap()
        );
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
            entry_hash: hash_blob(&vec![1]).unwrap(), // 407f958990678e2e9fb06758bc6520dae46d838d39948a4c51a5b19bd079293d
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

        assert_eq!(
            calculated_tree_hash.as_ref(),
            hash_tree(&dummy_tree).unwrap()
        );
        assert_eq!(
            calculated_tree_hash.as_ref(),
            hex::decode(expected_tree_hash).unwrap()
        );
    }

    #[test]
    fn test_tree_hash() {
        let sled  = sled::Config::new().temporary(true).open().unwrap();
        let backend = env::var("BACKEND").unwrap_or("mem".to_string());
        let mut storage = match backend.as_str() {
            "mem" => empty_mem_storage(),
            "sled" => get_sled_storage(sled.deref().clone()),
            _ => { panic!()}
        };
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

        let tree = storage.current_stage_tree.unwrap();

        let hash = hash_tree(&tree).unwrap();

        assert_eq!([0xDB, 0xAE, 0xD7, 0xB6], hash[0..4]);
    }

    #[test]
    fn test_commit_hash() {
        let sled  = sled::Config::new().temporary(true).open().unwrap();
        let backend = env::var("BACKEND").unwrap_or("mem".to_string());
        let mut storage = match backend.as_str() {
            "mem" => empty_mem_storage(),
            "sled" => get_sled_storage(sled.deref().clone()),
            _ => {panic!()}
        };

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
        let tree = storage.get_staged_root().unwrap();
        let hash = hash_tree(&tree).unwrap();
        get_short_hash(&hash)
    }

    #[test]
    fn test_examples_from_article_about_storage() {
        let sled  = sled::Config::new().temporary(true).open().unwrap();
        let backend = env::var("BACKEND").unwrap_or("mem".to_string());
        let mut storage = match backend.as_str() {
            "mem" => empty_mem_storage(),
            "sled" => get_sled_storage(sled.deref().clone()),
            _ => {panic!()}
        };

        storage.set(&vec!["a".to_string()], &vec![1]);
        storage.apply_actions_to_staging_area();
        let root = get_staged_root_short_hash(&mut storage);
        println!("SET [a] = 1\nROOT: {}", root);
        println!("CONTENT {}", storage.get_staged_entries());
        assert_eq!(root, "d49a53".to_string());

        storage.set(&vec!["b".to_string(), "c".to_string()], &vec![1]);
        storage.apply_actions_to_staging_area();
        let root = get_staged_root_short_hash(&mut storage);
        println!("\nSET [b,c] = 1\nROOT: {}", root);
        print!("{}", storage.get_staged_entries());
        assert_eq!(root, "ed8adf".to_string());

        storage.set(&vec!["b".to_string(), "d".to_string()], &vec![2]);
        storage.apply_actions_to_staging_area();
        let root = get_staged_root_short_hash(&mut storage);
        println!("\nSET [b,d] = 2\nROOT: {}", root);
        print!("{}", storage.get_staged_entries());
        assert_eq!(root, "437186".to_string());

        storage.set(&vec!["a".to_string()], &vec![2]);
        storage.apply_actions_to_staging_area();
        let root = get_staged_root_short_hash(&mut storage);
        println!("\nSET [a] = 2\nROOT: {}", root);
        print!("{}", storage.get_staged_entries());
        assert_eq!(root, "0d78b3".to_string());

        let entries = storage.get_staged_entries();
        let commit_hash = storage
            .commit(0, "Tezedge".to_string(), "persist changes".to_string())
            .unwrap();
        storage.apply_actions_to_staging_area();
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
        let sled  = sled::Config::new().temporary(true).open().unwrap();
        let backend = env::var("BACKEND").unwrap_or("mem".to_string());
        let mut storage = match backend.as_str() {
            "mem" => empty_mem_storage(),
            "sled" => get_sled_storage(sled.deref().clone()),
            _ => {panic!()}
        };
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

    // #[test]
    // fn test_empty_commit_should_be_rejeted() {
    //     let mut storage = get_empty_storage();
    //     storage.commit(0, "Tezos".to_string(), "Genesis".to_string());

    //     storage.set(&context_key!("a/b/c"), &vec![97]);
    //     let commit = storage.commit(1, "Tezos".to_string(), "1".to_string());
    //     let empty_commit = storage.commit(2, "Tezos".to_string(), "2".to_string());

    //     assert_eq!(commit.unwrap(), empty_commit.unwrap());
    // }

    #[test]
    fn test_get() {

        let db = Arc::new(RwLock::new(HashMap::new()));
        let sled  = sled::Config::new().temporary(true).open().unwrap();
        let backend = env::var("BACKEND").unwrap_or("mem".to_string());


        let commit1;
        let commit2;
        let key_abc: &ContextKey = &vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let key_abx: &ContextKey = &vec!["a".to_string(), "b".to_string(), "x".to_string()];
        let key_eab: &ContextKey = &vec!["e".to_string(), "a".to_string(), "b".to_string()];
        let key_az: &ContextKey = &vec!["a".to_string(), "z".to_string()];
        let key_d: &ContextKey = &vec!["d".to_string()];

        {
            let mut storage = match backend.as_str() {
                "mem" => get_mem_storage(db.clone()),
                "sled" => get_sled_storage(sled.deref().clone()),
                _ => {panic!()}
            };

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

        let mut storage = match backend.as_str() {
            "mem" => get_mem_storage(db),
            "sled" => get_sled_storage(sled.deref().clone()),
            _ => {panic!()}
        };

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
        let sled  = sled::Config::new().temporary(true).open().unwrap();
        let backend = env::var("BACKEND").unwrap_or("mem".to_string());
        let mut storage = match backend.as_str() {
            "mem" => empty_mem_storage(),
            "sled" => get_sled_storage(sled.deref().clone()),
            _ => {panic!()}
        };

        let key_abc: &ContextKey = &vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let key_abx: &ContextKey = &vec!["a".to_string(), "b".to_string(), "x".to_string()];

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
    fn test_dirmem() {
        let sled  = sled::Config::new().temporary(true).open().unwrap();
        let backend = env::var("BACKEND").unwrap_or("mem".to_string());
        let mut storage = match backend.as_str() {
            "mem" => empty_mem_storage(),
            "sled" => get_sled_storage(sled.deref().clone()),
            _ => {panic!()}
        };

        let key_abc: &ContextKey = &vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let key_ab: &ContextKey = &vec!["a".to_string(), "b".to_string()];
        let key_a: &ContextKey = &vec!["a".to_string()];

        assert_eq!(storage.dirmem(&key_a).unwrap(), false);
        assert_eq!(storage.dirmem(&key_ab).unwrap(), false);
        assert_eq!(storage.dirmem(&key_abc).unwrap(), false);
        storage.set(key_abc, &vec![1u8, 2u8]);
        assert_eq!(storage.dirmem(&key_a).unwrap(), true);
        assert_eq!(storage.dirmem(&key_ab).unwrap(), true);
        assert_eq!(storage.dirmem(&key_abc).unwrap(), false);
        storage.delete(key_abc);
        assert_eq!(storage.dirmem(&key_a).unwrap(), false);
        assert_eq!(storage.dirmem(&key_ab).unwrap(), false);
        assert_eq!(storage.dirmem(&key_abc).unwrap(), false);
    }

    #[test]
    fn test_copy() {
        let sled  = sled::Config::new().temporary(true).open().unwrap();
        let backend = env::var("BACKEND").unwrap_or("mem".to_string());
        let mut storage = match backend.as_str() {
            "mem" => empty_mem_storage(),
            "sled" => get_sled_storage(sled.deref().clone()),
            _ => {panic!()}
        };

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
        let sled  = sled::Config::new().temporary(true).open().unwrap();
        let backend = env::var("BACKEND").unwrap_or("mem".to_string());
        let mut storage = match backend.as_str() {
            "mem" => empty_mem_storage(),
            "sled" => get_sled_storage(sled.deref().clone()),
            _ => {panic!()}
        };
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
        let sled  = sled::Config::new().temporary(true).open().unwrap();
        let backend = env::var("BACKEND").unwrap_or("mem".to_string());
        let mut storage = match backend.as_str() {
            "mem" => empty_mem_storage(),
            "sled" => get_sled_storage(sled.deref().clone()),
            _ => {panic!()}
        };
        let key_abc: &ContextKey = &vec!["a".to_string(), "b".to_string(), "c".to_string()];
        storage.set(key_abc, &vec![2_u8]);
        let commit1 = storage.commit(0, "".to_string(), "".to_string()).unwrap();
        storage.delete(key_abc);
        let _commit2 = storage.commit(0, "".to_string(), "".to_string()).unwrap();

        assert_eq!(vec![2_u8], storage.get_history(&commit1, &key_abc).unwrap());
    }

    #[test]
    fn test_delete_in_separate_commit() {
        let sled  = sled::Config::new().temporary(true).open().unwrap();
        let backend = env::var("BACKEND").unwrap_or("mem".to_string());
        let mut storage = match backend.as_str() {
            "mem" => empty_mem_storage(),
            "sled" => get_sled_storage(sled.deref().clone()),
            _ => {panic!()}
        };
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
        let db = Arc::new(RwLock::new(HashMap::new()));
        let sled  = sled::Config::new().temporary(true).open().unwrap();

        let commit1;
        let commit2;
        let key_abc: &ContextKey = &vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let key_abx: &ContextKey = &vec!["a".to_string(), "b".to_string(), "x".to_string()];

        {
            let backend = env::var("BACKEND").unwrap_or("mem".to_string());
            let mut storage = match backend.as_str() {
                "mem" => get_mem_storage(db.clone()),
                "sled" => get_sled_storage(sled.deref().clone()),
                _ => {panic!()}
            };
            storage.set(key_abc, &vec![1u8]).unwrap();
            storage.set(key_abx, &vec![2u8]).unwrap();
            commit1 = storage.commit(0, "".to_string(), "".to_string()).unwrap();

            storage.set(key_abc, &vec![3u8]).unwrap();
            storage.set(key_abx, &vec![4u8]).unwrap();
            commit2 = storage.commit(0, "".to_string(), "".to_string()).unwrap();
        }


        let backend = env::var("BACKEND").unwrap_or("mem".to_string());
        let mut storage = match backend.as_str() {
            "mem" => get_mem_storage(db),
            "sled" => get_sled_storage(sled.deref().clone()),
            _ => {panic!()}
        };
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
        let db = Arc::new(RwLock::new(HashMap::new()));
        let sled  = sled::Config::new().temporary(true).open().unwrap();
        let backend = env::var("BACKEND").unwrap_or("mem".to_string());
        let key_abc: &ContextKey = &vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let commit1;
        {
            let mut storage = match backend.as_str() {
                "mem" => get_mem_storage(db.clone()),
                "sled" => get_sled_storage(sled.deref().clone()),
                _ => {panic!()}
            };
            let key_abx: &ContextKey = &vec!["a".to_string(), "b".to_string(), "x".to_string()];
            storage.set(key_abc, &vec![2_u8]).unwrap();
            storage.set(key_abx, &vec![3_u8]).unwrap();
            commit1 = storage.commit(0, "".to_string(), "".to_string()).unwrap();
        }

        let mut storage = match backend.as_str() {
            "mem" => get_mem_storage(db.clone()),
            "sled" => get_sled_storage(sled.deref().clone()),
            _ => {panic!()}
        };
        assert_eq!(vec![2_u8], storage.get_history(&commit1, &key_abc).unwrap());
    }

    /*Test a DB error by writing into a read-only database.
    #[test]
    fn test_db_error() {
        let db_name = "ms_test_db_error";
        {
            clean_db(db_name);
            let cache = Cache::new_lru_cache(32 * 1024 * 1024).unwrap();
            get_storage(db_name, &cache);
        }

        let db = DB::open_for_read_only(&Options::default(), get_db_name(db_name), true).unwrap();
        let mut storage = MerkleStorage::new(Arc::new(RocksDBBackend::new(Arc::new(db),MerkleStorage::name())));
        storage.set(&vec!["a".to_string()], &vec![1u8]);
        let res = storage.commit(0, "".to_string(), "".to_string());

        assert!(matches!(res.err().unwrap(), MerkleError::DBError { .. }));
    }*/

    // Test getting entire tree in string format for JSON RPC
    #[test]
    fn test_get_context_tree_by_prefix() {
        let backend = env::var("BACKEND").unwrap_or("mem".to_string());
        let db = Arc::new(RwLock::new(HashMap::new()));
        let sled  = sled::Config::new().temporary(true).open().unwrap();

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

        let mut storage = match backend.as_str() {
            "mem" => get_mem_storage(db.clone()),
            "sled" => get_sled_storage(sled.deref().clone()),
            _ => {panic!()}
        };
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

    // TODO: use mock_instant crate or something like and enable this unit test
    // #[test]
    // fn test_block_latenices() {
    //     let mut storage = get_empty_storage();

    //     let t = |milis: u64| Instant::now() - Duration::from_nanos(milis * 1000);

    //     storage.update_execution_stats("Get".to_string(), None, &t(10));
    //     storage.update_execution_stats("Set".to_string(), None, &t(20));
    //     storage.update_execution_stats("Commit".to_string(), None, &t(30));

    //     assert_eq!(storage.get_block_latency(0).unwrap() / 1000, 60);

    //     storage.update_execution_stats("Set".to_string(), None, &t(6));
    //     storage.update_execution_stats("Commit".to_string(), None, &t(60));

    //     assert_eq!(storage.get_block_latency(0).unwrap() / 1000, 66);
    //     assert_eq!(storage.get_block_latency(1).unwrap() / 1000, 60);
    // }
}
