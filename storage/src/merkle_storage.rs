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
use serde::Deserialize;
use serde::Serialize;

use crypto::hash::HashType;

use crate::persistent::{default_table_options, KeyValueSchema, KeyValueStoreWithSchema};
use crate::persistent;
use crate::persistent::BincodeEncoded;
use crate::persistent::database::RocksDBStats;

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

pub type MerkleStorageKV = dyn KeyValueStoreWithSchema<MerkleStorage> + Sync + Send;

pub struct MerkleStorage {
    /// tree with current staging area (currently checked out context)
    current_stage_tree: Option<Tree>,
    current_stage_tree_hash: Option<EntryHash>,
    db: Arc<MerkleStorageKV>,
    /// all entries in current staging area
    staged: Vec<(EntryHash, Entry)>,
    last_commit_hash: Option<EntryHash>,
    map_stats: MerkleMapStats,
    /// divide this by the next field to get avg time spent in _set
    cumul_set_exec_time: f64,
    set_exec_times: u64,
    /// first N measurements to discard
    set_exec_times_to_discard: u64,
    /// list of all actions done on staging area
    actions: Arc<Vec<Action>>,
    /// list of context hashes after each Action step
    staging_context_hashes: Vec<EntryHash>,
}

#[derive(Debug, Fail)]
pub enum MerkleError {
    /// External libs errors
    #[fail(display = "RocksDB error: {:?}", error)]
    DBError { error: persistent::database::DBError },
    #[fail(display = "Serialization error: {:?}", error)]
    SerializationError { error: bincode::Error },

    /// Internal unrecoverable bugs that should never occur
    #[fail(display = "No root retrieved for this commit!")]
    CommitRootNotFound,
    #[fail(display = "Cannot commit without a predecessor!")]
    MissingAncestorCommit,
    #[fail(display = "There is a commit or three under key {:?}, but not a value!", key)]
    ValueIsNotABlob { key: String },
    #[fail(display = "Found wrong structure. Was looking for {}, but found {}", sought, found)]
    FoundUnexpectedStructure { sought: String, found: String },
    #[fail(display = "Entry not found! Hash={}", hash)]
    EntryNotFound { hash: String },

    /// Wrong user input errors
    #[fail(display = "No value under key {:?}.", key)]
    ValueNotFound { key: String },
    #[fail(display = "Cannot search for an empty key.")]
    KeyEmpty,
    #[fail(display = "Failed to convert hash to array: {}", error)]
    HashConversionError { error: TryFromSliceError },
}

impl From<persistent::database::DBError> for MerkleError {
    fn from(error: persistent::database::DBError) -> Self { MerkleError::DBError { error } }
}

impl From<bincode::Error> for MerkleError {
    fn from(error: bincode::Error) -> Self { MerkleError::SerializationError { error } }
}

impl From<TryFromSliceError> for MerkleError {
    fn from(error: TryFromSliceError) -> Self { MerkleError::HashConversionError { error } }
}

#[derive(Serialize, Debug, Clone, Copy)]
pub struct MerkleMapStats {
    staged_area_elems: u64,
    current_tree_elems: u64,
}

#[derive(Serialize, Debug, Clone, Copy)]
pub struct MerklePerfStats {
    pub avg_set_exec_time_ns: f64,
}

#[derive(Serialize, Debug, Clone)]
pub struct MerkleStorageStats {
    rocksdb_stats: RocksDBStats,
    map_stats: MerkleMapStats,
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
pub type StringTree = BTreeMap<String, StringTreeEntry>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StringTreeEntry {
    Tree(StringTree),
    Blob(String),
}
    fn encode_irmin_node_kind2(kind: &NodeKind) -> [u8; 8] {
        match kind {
            NodeKind::NonLeaf => [0, 0, 0, 0, 0, 0, 0, 0],
            NodeKind::Leaf => [255, 0, 0, 0, 0, 0, 0, 0],
        }
    }
    fn hash_tree2(tree: &Tree) -> Result<EntryHash, MerkleError> {
        let mut hasher = VarBlake2b::new(HASH_LEN).unwrap();

        hasher.update(&(tree.len() as u64).to_be_bytes());
        tree.iter().for_each(|(k, v)| {
            hasher.update(encode_irmin_node_kind2(&v.node_kind));
            hasher.update(&[k.len() as u8]);
            hasher.update(&k.clone().into_bytes());
            hasher.update(&(HASH_LEN as u64).to_be_bytes());
            hasher.update(&v.entry_hash);
        });

        Ok(hasher.finalize_boxed().as_ref().try_into()?)
    }

impl MerkleStorage {
    pub fn new(db: Arc<MerkleStorageKV>) -> Self {
        MerkleStorage {
            db,
            staged: Vec::new(),
            current_stage_tree: None,
            current_stage_tree_hash: None,
            last_commit_hash: None,
            map_stats: MerkleMapStats { staged_area_elems: 0, current_tree_elems: 0 },
            cumul_set_exec_time: 0.0,
            set_exec_times: 0,
            set_exec_times_to_discard: 20,
            actions: Arc::new(Vec::new()),
            staging_context_hashes: Vec::new(),
        }
    }

    /// Get value from current staged root
    pub fn get(&mut self, key: &ContextKey) -> Result<ContextValue, MerkleError> {
        let root = &self.get_staged_root()?;
        let root_hash = self.hash_tree(&root)?;

        self.get_from_tree(&root_hash, key)
    }

    /// Get value. Staging area is checked first, then last (checked out) commit.
    pub fn get_by_prefix(&mut self, prefix: &ContextKey) -> Result<Option<Vec<(ContextKey, ContextValue)>>, MerkleError> {
        let root = self.get_staged_root()?;
        self._get_key_values_by_prefix(root, prefix)
    }

    /// Get value from historical context identified by commit hash.
    pub fn get_history(&self, commit_hash: &EntryHash, key: &ContextKey) -> Result<ContextValue, MerkleError> {
        let commit = self.get_commit(commit_hash)?;

        self.get_from_tree(&commit.root_hash, key)
    }

    fn get_from_tree(&self, root_hash: &EntryHash, key: &ContextKey) -> Result<ContextValue, MerkleError> {
        let mut full_path = key.clone();
        let file = full_path.pop().ok_or(MerkleError::KeyEmpty)?;
        let path = full_path;
        // find tree by path
        let root = self.get_tree(root_hash)?;
        let node = self.find_tree(&root, &path)?;

        // get file node from tree
        let node = match node.get(&file) {
            None => return Err(MerkleError::ValueNotFound { key: self.key_to_string(key) }),
            Some(entry) => entry,
        };
        // get blob by hash
        match self.get_entry(&node.entry_hash)? {
            Entry::Blob(blob) => Ok(blob),
            _ => Err(MerkleError::ValueIsNotABlob { key: self.key_to_string(key) })
        }
    }

    // TODO: recursion is risky (stack overflow) and inefficient, try to do it iteratively..
    fn get_key_values_from_tree_recursively(&self, path: &str, entry: &Entry, entries: &mut Vec<(ContextKey, ContextValue)>) -> Result<(), MerkleError> {
        match entry {
            Entry::Blob(blob) => {
                // push key-value pair
                entries.push((self.string_to_key(path), blob.to_vec()));
                Ok(())
            }
            Entry::Tree(tree) => {
                // Go through all descendants and gather errors. Remap error if there is a failure
                // anywhere in the recursion paths. TODO: is revert possible?
                tree.iter().map(|(key, child_node)| {
                    let fullpath = path.to_owned() + "/" + key;
                    match self.get_entry(&child_node.entry_hash) {
                        Err(_) => Ok(()),
                        Ok(entry) => self.get_key_values_from_tree_recursively(&fullpath, &entry, entries),
                    }
                }).find_map(|res| {
                    match res {
                        Ok(_) => None,
                        Err(err) => Some(Err(err)),
                    }
                }).unwrap_or(Ok(()))
            }
            Entry::Commit(commit) => {
                match self.get_entry(&commit.root_hash) {
                    Err(err) => Err(err),
                    Ok(entry) => self.get_key_values_from_tree_recursively(path, &entry, entries),
                }
            }
        }
    }

    /// Go recursively down the tree from Entry, build string tree and return it
    /// (or return hex value if Blob)
    fn get_context_recursive(&self, path: &str, entry: &Entry) -> Result<StringTreeEntry, MerkleError> {
        match entry {
            Entry::Blob(blob) => {
                Ok(StringTreeEntry::Blob(hex::encode(blob).to_string()))
            }
            Entry::Tree(tree) => {
                // Go through all descendants and gather errors. Remap error if there is a failure
                // anywhere in the recursion paths. TODO: is revert possible?
                let mut new_tree = StringTree::new();
                for (key, child_node) in tree.iter() {
                    let fullpath = path.to_owned() + "/" + key;
                    let e = self.get_entry(&child_node.entry_hash)?;
                    new_tree.insert(key.to_owned(), self.get_context_recursive(&fullpath, &e)?);
                }
                Ok(StringTreeEntry::Tree(new_tree))
            }
            Entry::Commit(_) => Err(MerkleError::FoundUnexpectedStructure {
                sought: "Tree/Blob".to_string(),
                found: "Commit".to_string(),
            })
        }
    }

    /// Get context tree under given prefix in string form (for JSON)
    pub fn get_context_tree_by_prefix(&self, context_hash: &EntryHash, prefix: &ContextKey) -> Result<StringTree, MerkleError> {
        let mut out = StringTree::new();
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
            out.insert(key.to_owned(), self.get_context_recursive(&fullpath, &entry)?);
        }

        Ok(out)
    }

    /// Construct Vec of all context key-values under given prefix
    pub fn get_key_values_by_prefix(&self, context_hash: &EntryHash, prefix: &ContextKey) -> Result<Option<Vec<(ContextKey, ContextValue)>>, MerkleError> {
        let commit = self.get_commit(context_hash)?;
        let root_tree = self.get_tree(&commit.root_hash)?;
        self._get_key_values_by_prefix(root_tree, prefix)
    }

    fn _get_key_values_by_prefix(&self, root_tree: Tree, prefix: &ContextKey) -> Result<Option<Vec<(ContextKey, ContextValue)>>, MerkleError> {
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
        self.current_stage_tree = Some(self.get_tree(&commit.root_hash)?);
        self.current_stage_tree_hash = Some(commit.root_hash);
        self.map_stats.current_tree_elems = self.current_stage_tree.as_ref().unwrap().len() as u64;
        self.last_commit_hash = Some(*context_hash);
        self.staged = Vec::new();
        self.map_stats.staged_area_elems = 0;
        Ok(())
    }

    /// Take the current changes in the staging area, create a commit and persist all changes
    /// to database under the new commit. Return last commit if there are no changes, that is
    /// empty commits are not allowed.
    pub fn commit(&mut self,
                  time: u64,
                  author: String,
                  message: String,
    ) -> Result<EntryHash, MerkleError> {
        //println!("commit()");
        self.do_the_thing()?;

        let staged_root = self.get_staged_root()?;
        let staged_root_hash = self.hash_tree(&staged_root)?;
        let parent_commit_hash = self.last_commit_hash;

        let new_commit = Commit {
            root_hash: staged_root_hash,
            parent_commit_hash,
            time,
            author,
            message,
        };
        let entry = Entry::Commit(new_commit.clone());


        self.put_to_staging_area(&self.hash_commit(&new_commit)?, entry.clone());
        self.persist_staged_entry_to_db(&entry)?;
        self.staged = Vec::new();
        self.map_stats.staged_area_elems = 0;
        let last_commit_hash = self.hash_commit(&new_commit)?;
        self.last_commit_hash = Some(last_commit_hash);
        Ok(last_commit_hash)
    }

    /// Set key/val to the staging area.
    pub fn set(&mut self, key: &ContextKey, value: &ContextValue) -> Result<(), MerkleError> {
        //let root = self.get_staged_root()?;
        // store action
        //println!("set()");
        let act = Arc::make_mut(&mut self.actions);
        act.push(Action::Set( SetAction{ key: key.to_vec(), value: value.to_vec() } ));
        //let new_root_hash = &self._set(&root, key, value)?;
        //println!("_set() returned new_root_hash={}, set as current_stage_tree", new_root_hash[0]);
        //self.current_stage_tree = Some(self.get_tree(new_root_hash)?);
        //self.map_stats.current_tree_elems = self.current_stage_tree.as_ref().unwrap().len() as u64;
        Ok(())
    }

    /// Walk down the tree to find key, set new value and walk back up recalculating hashes -
    /// return new top hash of tree. Note: no writes to DB yet
    fn _set(&mut self, root: &Tree, key: &ContextKey, value: &ContextValue) -> Result<EntryHash, MerkleError> {
        let blob_hash = self.hash_blob(&value)?;
        self.put_to_staging_area(&blob_hash, Entry::Blob(value.clone()));
        //println!("put blobhash={} in staging", blob_hash[0]);
        let new_node = Node { entry_hash: blob_hash, node_kind: NodeKind::Leaf };
        let instant = Instant::now();
        let rv = self.compute_new_root_with_change(root, &key, Some(new_node));
        let elapsed = instant.elapsed().as_nanos() as f64;
        if self.set_exec_times >= self.set_exec_times_to_discard.into() {
            self.cumul_set_exec_time += elapsed;
        }
        self.set_exec_times += 1;
        rv
    }

    /// Delete an item from the staging area.
    pub fn delete(&mut self, key: &ContextKey) -> Result<(), MerkleError> {
        //let root = self.get_staged_root()?;
        // store action
        let act = Arc::make_mut(&mut self.actions);
        act.push(Action::Remove( RemoveAction{ key: key.to_vec() } ));
        //let new_root_hash = &self._delete(&root, key)?;
        //self.current_stage_tree = Some(self.get_tree(new_root_hash)?);
        //self.map_stats.current_tree_elems = self.current_stage_tree.as_ref().unwrap().len() as u64;
        Ok(())
    }

    fn _delete(&mut self, root: &Tree, key: &ContextKey) -> Result<EntryHash, MerkleError> {
        if key.is_empty() { return self.hash_tree(root); }

        self.compute_new_root_with_change(root, &key, None)
    }

    /// Copy subtree under a new path.
    /// TODO Consider copying values!
    pub fn copy(&mut self, from_key: &ContextKey, to_key: &ContextKey) -> Result<(), MerkleError> {
        //let root = self.get_staged_root()?;
        // store action
        let act = Arc::make_mut(&mut self.actions);
        act.push(Action::Copy( CopyAction{ from_key: from_key.to_vec(), to_key: to_key.to_vec() } ));
        //let new_root_hash = &self._copy(&root, from_key, to_key)?;
        //self.current_stage_tree = Some(self.get_tree(new_root_hash)?);
        //self.map_stats.current_tree_elems = self.current_stage_tree.as_ref().unwrap().len() as u64;
        Ok(())
    }

    fn _copy(&mut self, root: &Tree, from_key: &ContextKey, to_key: &ContextKey) -> Result<EntryHash, MerkleError> {
        let source_tree = self.find_tree(root, &from_key)?;
        let source_tree_hash = self.hash_tree(&source_tree)?;
        Ok(self.compute_new_root_with_change(
            &root, &to_key, Some(self.get_non_leaf(source_tree_hash)))?)
    }

    fn ensure_stage_tree_exists(&mut self) -> Result<(), MerkleError> {
        match &self.current_stage_tree {
            None => {
                let tree = Tree::new();
                self.current_stage_tree = Some(tree.clone());
                let hash = self.hash_tree(&tree)?;
                self.current_stage_tree_hash = Some(hash);
                self.staging_context_hashes.push(hash);
                self.put_to_staging_area(&hash, Entry::Tree(tree.clone()));
            }
            Some(_tree) => (),
        }
        Ok(())
    }
    fn do_the_thing(&mut self) -> Result<(), MerkleError> {
        //called in commit()

        //walk through actions list and apply actions sequentially
        
        //build self.staged
        //
        //set current_stage_tree to result of compute_new_root (get_tree(rv))
                //-> actually no need to do it on every action
        //
        //persist Commit to DB using self.staged HashMap

        self.ensure_stage_tree_exists()?;
        //let root = self.get_staged_root()?;


        let actions = self.actions.clone();
        //println!("do_the_thing: len(actions)={}", actions.len());
        for action in actions.iter() {
            match action {
                Action::Set(set) =>  {
       // let blob_hash = self.hash_blob(&value)?;
       // self.put_to_staging_area(&blob_hash, Entry::Blob(value.clone()));
       // let new_node = Node { entry_hash: blob_hash, node_kind: NodeKind::Leaf };
       // let rv = self.compute_new_root_with_change(root, &key, Some(new_node));
        //
                    //TODO DONT CLONE let root = &self.current_stage_tree.as_ref().unwrap(); 
                    //let root = self.current_stage_tree.clone().unwrap();
                    //println!("Action::Set");
                    let root_hash = self.current_stage_tree_hash.unwrap();
                    //println!("do_the_thing(), root_hash={}", root_hash[0]);
                    let key = &set.key;
                    let blob_hash = self.hash_blob(&set.value)?;
                    //println!("do_the_thing(), blob_hash={}", blob_hash[0]);
                    self.staged.push((blob_hash, Entry::Blob(set.value.clone())));
                    let new_node = Node { entry_hash: blob_hash, node_kind: NodeKind::Leaf };


                    ////dump staging
                    //println!("dumping staging");
                    //for (_, staged_entry) in self.staged.iter().enumerate() {
                    //    let (hash, entry) = staged_entry;
                    //    println!("{}", hash[0]);
                    //}
                    //println!("end of dump");

                    //TODO inefficient - instead of pushing here just don't remove this entry on
                    //commit() (where we set self.staged to Vec::new())
                    self.staged.push((root_hash, self.get_entry(&root_hash)?));
                    let rv = self.compute_new_root_with_change_alt(&root_hash, &key, Some(new_node))?;

                    ////dump staging
                    //println!("dumping staging");
                    //for (_, staged_entry) in self.staged.iter().enumerate() {
                    //    let (hash, entry) = staged_entry;
                    //    println!("{}", hash[0]);
                    //}
                    //println!("end of dump");


                    // now put Tree of hash rv into current_staging_tree
                    // TODO: can be optimized (unfortunately get_tree() currently clones tree)
                    // e.g. make current_stage_tree an index into staged Vec
                    //println!("compute_alt returned hash={}", rv[0]);
                    self.current_stage_tree = Some(self.get_tree(&rv)?);
                    self.current_stage_tree_hash = Some(rv);
                    self.staging_context_hashes.push(rv);

                    // TODO: if find_tree() doesnt find tree in staged, it will panic (unwrap)
                    //      is there a scenario where it's only in DB, not in staged?
                    //      well I think yes, after checkout() staged stores only tree of hashes, while blobs are in
                    //      DB..
                }

                Action::Copy(copy) => {
                    //println!("Action::Copy");
                  //  let root_hash = self.current_stage_tree_hash.unwrap();
                  //  self.staged.push((root_hash, self.get_entry(&root_hash)?));
                  //  let root_idx = self.staged_get_idx(&root_hash).unwrap();
                  //  let idx = self.find_tree_staging(root_idx, root_hash, copy.from_key)?;
                  //  let source_tree;
                  //  if idx.is_none() {
                  //      let source_tree = self.find_tree(root, &from_key)?;
                  //      let source_tree_hash = self.hash_tree(&source_tree)?;
                  //  }
                  
                    //original impl, slow
                    let root_hash = self.current_stage_tree_hash.unwrap();
                    let root = self.get_entry(&root_hash)?;
                    let rv;
                    if let Entry::Tree(root) = root {
                        let source_tree = self.find_tree(&root, &copy.from_key)?;
                        let source_tree_hash = self.hash_tree(&source_tree)?;
                        rv = self.compute_new_root_with_change(
                            &root, &copy.to_key, Some(self.get_non_leaf(source_tree_hash)))?;
                    } else {
                        panic!("Action Copy(): not a tree");
                    }
                    self.current_stage_tree = Some(self.get_tree(&rv)?);
                    self.current_stage_tree_hash = Some(rv);
                    self.staging_context_hashes.push(rv);
                    //println!("End of Copy");
                }

                Action::Remove(remove) => {
                    let root_hash = self.current_stage_tree_hash.unwrap();
                    let root = self.get_entry(&root_hash)?;
                    let rv;
                    if let Entry::Tree(root) = root {
                        rv = self.compute_new_root_with_change(&root, &remove.key, None)?;
                    } else {
                        panic!("Action Remove(): not a tree");
                    }
                    self.current_stage_tree = Some(self.get_tree(&rv)?);
                    self.current_stage_tree_hash = Some(rv);
                    self.staging_context_hashes.push(rv);
                }
            }
        }
        
        // clear list of actions
        self.actions = Arc::new(Vec::new());

        Ok(())
    }

    fn compute_new_root_with_change_alt(&mut self,
                                    root_hash: &EntryHash,
                                    key: &[String],
                                    new_node: Option<Node>,
    ) -> Result<EntryHash, MerkleError> {
        let root_idx = self.staged_get_idx(&root_hash).unwrap();

        //println!("compute_alt with keylen={}, and node hash={}", key.len(),
         //           new_node.clone().unwrap().entry_hash[0]);
        assert_eq!(key.is_empty(), false);
        if key.is_empty() {
            match new_node {
                Some(n) =>  {
                    //println!("returning hash={}", n.clone().entry_hash[0]);
                    return Ok(n.entry_hash);
                }
                None => {
                    return Ok(*root_hash);
                }
            }
        }

        let last = key.last().unwrap();
        let path = &key[..key.len() - 1];

        // find tree by path and get new copy of it
        let idx = self.find_tree_staging(root_idx, root_hash, path)?;
        let idx = idx.or_else(|| { 
            // node doesn't exist or is Blob, create empty tree
            let tree = Tree::new();
            let hash = self.hash_tree(&tree).unwrap();
            self.put_to_staging_area(&hash, Entry::Tree(tree.clone()));
            return self.staged_get_idx(&hash);
        });

        //println!("modifying tree in place");
        match idx {
            Some(idx) => {
                let (ref mut tree_hash, ref mut tree) = self.staged[idx];
                if let Entry::Tree(tree) = tree {
                    // make the modification of tree at key, in place
                    match new_node {
                        None => (tree).remove(last),
                        Some(new_node) => {
                            tree.insert(last.clone(), new_node)
                        }
                    };
                    // calculate hash of modified tree
                    let new_tree_hash = hash_tree2(&tree)?;

                    // tree was modified in place so just update its hash in staged
                    // note: old tree is gone, will need to be recreated for backtracking
                    //self.update_hash_in_staged(&tree_hash, &new_tree_hash);

//                    let mut root_hash = *root_hash;
//                    if *tree_hash == root_hash {
//                        //assert_eq!(path.is_empty(), true);
//                        // tree we just modified was root - need to update root_hash to pass it to
//                        // compute_new_root below
//                        root_hash = new_tree_hash;
//                    }
                    //println!("changing hash={} to new tree hash={} in staging", tree_hash[0], new_tree_hash[0]);
                    //*tree_hash = new_tree_hash;
                    *tree_hash = new_tree_hash;

                    if tree.is_empty() {
                        // CHECK THIS BRANCH LATER
                        // last element was removed, delete this node
                        //println!("Tree is empty");
                        if path.is_empty() {
                            //println!("Also path is empty - returning root hash");
                            return Ok(*root_hash);
                        }
                        self.compute_new_root_with_change_alt(&root_hash, path, None)
                    } else {
                        if path.is_empty() {
                            //println!("path is empty - returning new_tree_hash");
                            return Ok(new_tree_hash);
                        }
                        self.compute_new_root_with_change_alt(
                            &root_hash, path, Some(self.get_non_leaf(new_tree_hash)))
                    }
                } else {
                    panic!("compute_alt: Entry is not a Tree");
                }
            },
            None => {
                // error getting tree from staged - should not happen
                panic!("compute_alt: idx is None");
            },
        }
        // example call chain for set()
        //
        // set(a/b, 2)
        // h = hash(2)
        // n = Node(h)
        // compute(root = staged_tree, a/b, n)
        //      last = b
        //      path = a
        //      t = find_tree(root, a)
        //      t.insert(b, n)
        //      new_t_hash = hash(t)
        //      put_to_staging(new_t_hash, t)
        //      compute(root, a, Node(new_t_hash))
        //          last = a
        //          path == ''
        //          t = find_tree(root, '')   // just returns root
        //          t.insert(a, Node(new_t_hash)   // t is root (staged tree)
        //          new_t_hash = hash(t)
        //          put_to_staging(new_t_hash, t)
        //          compute(root, '', Node(new_t_hash)
        //              return new_t_hash
        //
    }

    // returns index to self.staged with found subtree and its hash
    fn find_tree_staging(&mut self, root_idx: usize, root_hash: &EntryHash, key: &[String]) -> Result<Option<usize>, MerkleError> {
        //println!("find_tree_staging() with keylen={}", key.len());
        if key.is_empty() {
            return Ok(Some(root_idx));
        }

        let (_, ref root) = self.staged[root_idx];
        let child_node = match root {
                Entry::Tree(root) => {
                    match root.get(key.first().unwrap()) {
                        Some(node) => node,
                        None =>  {
                            //println!("no node at key={}, returning None", key.first().unwrap());
                            return Ok(None);
                        }
                    }
                },
                _ => {
                    //println!("ERROR: root is not tree!");
                    return Ok(None); //TO DO revise this branch
                }
        };

        let mut entry_idx;
        let ehash = child_node.entry_hash.clone();
        match self.staged_get_idx(&child_node.entry_hash) {
            Some(idx) => {
                entry_idx = idx;
            }
            None => {
                self.put_to_staging_area(&ehash,
                                         self.get_entry_db(&ehash)?);
                entry_idx = self.staged_get_idx(&ehash).unwrap();
            }
        }

        let (entry_hash, ref entry) = self.staged[entry_idx];
        match entry {
            Entry::Tree(tree) => {
                if key.len() == 1 {
                    //println!("keylen=1, returning entry_idx");
                    return Ok(Some(entry_idx));
                } else {
                    //println!("entry with hash={} is tree, calling find_tree with keylen={}",
                     //   &ehash[0], &key[1..].len());
                    self.find_tree_staging(entry_idx, &entry_hash, &key[1..])
                }
            }
            Entry::Blob(_) => Ok(None),
            Entry::Commit { .. } => Err(MerkleError::FoundUnexpectedStructure {
                sought: "tree".to_string(),
                found: "commit".to_string(),
            })
        }
    }

    fn staged_get(&self, hash: &EntryHash) -> Option<&Entry> {
        for entry in self.staged.iter() {
            if entry.0 == *hash {
                return Some(&entry.1);
            }
        }
        return None;
    }
    // returns index to self.staged with found entry
    fn staged_get_idx(&self, hash: &EntryHash) -> Option<usize> {
        for (idx, entry) in self.staged.iter().enumerate() {
            if entry.0 == *hash {
                return Some(idx);
            }
        }
        return None;
    }
    fn update_hash_in_staged(&mut self, from_hash: &EntryHash, to_hash: &EntryHash) {
        for entry in self.staged.iter_mut() {
            if entry.0 == *from_hash {
                entry.0 = *to_hash;
            }
        }
    }





    /// Get a new tree with `new_entry_hash` put under given `key`.
    ///
    /// # Arguments
    ///
    /// * `root` - Tree to modify
    /// * `key` - path under which the changes takes place
    /// * `new_entry_hash` - None for deletion, Some for inserting a hash under the key.
    fn compute_new_root_with_change(&mut self,
                                    root: &Tree,
                                    key: &[String],
                                    new_node: Option<Node>,
    ) -> Result<EntryHash, MerkleError> {

        //println!("compute_new_root with keylen={}, and node hash=", key.len());
                    //new_node.clone().unwrap().entry_hash[0]);
        if key.is_empty() {
            match new_node {
                Some(n) =>  {
                    //println!("returning hash={}", n.clone().entry_hash[0]);
                    return Ok(n.entry_hash);
                }
                None => {
                    let tree_hash = self.hash_tree(root)?;
                    return Ok(self.get_non_leaf(tree_hash).entry_hash);
                }
            }
        }

        let last = key.last().unwrap();
        let path = &key[..key.len() - 1];
        // find tree by path and get new copy of it
        let mut tree = self.find_tree(root, path)?;

        //println!("modifying copy of tree");
        // make the modification at key
        match new_node {
            None => tree.remove(last),
            Some(new_node) => {
                tree.insert(last.clone(), new_node)
            }
        };

        if tree.is_empty() {
            // last element was removed, delete this node
            self.compute_new_root_with_change(root, path, None)
        } else {
            let new_tree_hash = self.hash_tree(&tree)?;
            // put new version of the tree to staging area
            // note: the old version is kept in staging area
            //println!("putting new tree hash={} to staging", new_tree_hash[0]);
            self.put_to_staging_area(&new_tree_hash, Entry::Tree(tree));
            self.compute_new_root_with_change(
                root, path, Some(self.get_non_leaf(new_tree_hash)))
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
        //println!("find_tree() with keylen={}", key.len());
        if key.is_empty() { 
            return Ok(root.clone()); 
        }

        // first get node at key
        let child_node = match root.get(key.first().unwrap()) {
            Some(hash) => hash,
            None => {
                //println!("no node at key={}, returning empty tree", key.first().unwrap());
                return Ok(Tree::new());
            }
        };

        //println!("find_tree: calling get_entry for hash={} at key.first={}",
         //        child_node.entry_hash[0], key.first().unwrap());
        // get entry by hash (from staged area or DB)
        match self.get_entry(&child_node.entry_hash)? {
            Entry::Tree(tree) => {
                //println!("entry with hash={} is tree, calling find_tree with keylen={}",
                 //       &child_node.entry_hash[0], &key[1..].len());
                self.find_tree(&tree, &key[1..])
            }
            Entry::Blob(_) => {
                //println!("entry with hash={} is blob, returning empty tree",
                 //   &child_node.entry_hash[0]);
                return Ok(Tree::new());
            }
            Entry::Commit { .. } => Err(MerkleError::FoundUnexpectedStructure {
                sought: "tree".to_string(),
                found: "commit".to_string(),
            })
        }
    }

    /// Get latest staged tree. If it's empty, init genesis  and return genesis root.
    fn get_staged_root(&mut self) -> Result<Tree, MerkleError> {
        match &self.current_stage_tree {
            None => {
                let tree = Tree::new();
                self.put_to_staging_area(&self.hash_tree(&tree)?, Entry::Tree(tree.clone()));
                self.map_stats.current_tree_elems = tree.len() as u64;
                Ok(tree)
            }
            Some(tree) => {
                self.map_stats.current_tree_elems = tree.len() as u64;
                Ok(tree.clone())
            }
        }
    }

    fn put_to_staging_area(&mut self, key: &EntryHash, value: Entry) {
        //TODO: check if exists already, otherwise we have duplicates
        self.staged.push((*key, value));
        self.map_stats.staged_area_elems = self.staged.len() as u64;
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
    fn get_entries_recursively(&self, entry: &Entry, batch: &mut WriteBatch) -> Result<(), MerkleError> {
        // add entry to batch
        self.db.put_batch(
            batch,
            &self.hash_entry(entry)?,
            &bincode::serialize(entry)?)?;

        match entry {
            Entry::Blob(_) => Ok(()),
            Entry::Tree(tree) => {
                // Go through all descendants and gather errors. Remap error if there is a failure
                // anywhere in the recursion paths. TODO: is revert possible?
                tree.iter().map(|(_, child_node)| {
                    match self.staged_get(&child_node.entry_hash) {
                        None => Ok(()),
                        Some(entry) => self.get_entries_recursively(entry, batch),
                    }
                }).find_map(|res| {
                    match res {
                        Ok(_) => None,
                        Err(err) => Some(Err(err)),
                    }
                }).unwrap_or(Ok(()))
            }
            Entry::Commit(commit) => {
                match self.get_entry(&commit.root_hash) {
                    Err(err) => Err(err),
                    Ok(entry) => self.get_entries_recursively(&entry, batch),
                }
            }
        }
    }

    fn hash_entry(&self, entry: &Entry) -> Result<EntryHash, MerkleError> {
        match entry {
            Entry::Commit(commit) => self.hash_commit(&commit),
            Entry::Tree(tree) => self.hash_tree(&tree),
            Entry::Blob(blob) => self.hash_blob(blob),
        }
    }

    fn hash_commit(&self, commit: &Commit) -> Result<EntryHash, MerkleError> {
        let mut hasher = VarBlake2b::new(HASH_LEN).unwrap();
        hasher.update(&(HASH_LEN as u64).to_be_bytes());
        hasher.update(&commit.root_hash);

        if commit.parent_commit_hash.is_none() {
            hasher.update(&(0 as u64).to_be_bytes());
        } else {
            hasher.update(&(1 as u64).to_be_bytes()); // # of parents; we support only 1
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

    fn hash_tree(&self, tree: &Tree) -> Result<EntryHash, MerkleError> {
        let mut hasher = VarBlake2b::new(HASH_LEN).unwrap();

        hasher.update(&(tree.len() as u64).to_be_bytes());
        tree.iter().for_each(|(k, v)| {
            hasher.update(&self.encode_irmin_node_kind(&v.node_kind));
            hasher.update(&[k.len() as u8]);
            hasher.update(&k.clone().into_bytes());
            hasher.update(&(HASH_LEN as u64).to_be_bytes());
            hasher.update(&v.entry_hash);
        });

        Ok(hasher.finalize_boxed().as_ref().try_into()?)
    }

    fn hash_blob(&self, blob: &ContextValue) -> Result<EntryHash, MerkleError> {
        let mut hasher = VarBlake2b::new(HASH_LEN).unwrap();
        hasher.update(&(blob.len() as u64).to_be_bytes());
        hasher.update(blob);

        Ok(hasher.finalize_boxed().as_ref().try_into()?)
    }

    fn encode_irmin_node_kind(&self, kind: &NodeKind) -> [u8; 8] {
        match kind {
            NodeKind::NonLeaf => [0, 0, 0, 0, 0, 0, 0, 0],
            NodeKind::Leaf => [255, 0, 0, 0, 0, 0, 0, 0],
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
            None => Err(MerkleError::EntryNotFound { hash: HashType::ContextHash.bytes_to_string(hash) }),
            Some(entry_bytes) => Ok(bincode::deserialize(&entry_bytes)?),
        }
    }
    /// Get entry from staging area or look up in DB if not found
    fn get_entry(&self, hash: &EntryHash) -> Result<Entry, MerkleError> {
        match self.staged_get(hash) {
            None => {
                let entry_bytes = self.db.get(hash)?;
                match entry_bytes {
                    None => Err(MerkleError::EntryNotFound { hash: HashType::ContextHash.bytes_to_string(hash) }),
                    Some(entry_bytes) => Ok(bincode::deserialize(&entry_bytes)?),
                }
            }
            Some(entry) => Ok(entry.clone()),
        }
    }

    fn get_non_leaf(&self, hash: EntryHash) -> Node {
        Node { node_kind: NodeKind::NonLeaf, entry_hash: hash }
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
    pub fn get_merkle_stats(&self) -> Result<MerkleStorageStats, MerkleError> {
        let db_stats = self.db.get_mem_use_stats()?;
        let mut avg_set_exec_time_ns: f64 = 0.0;
        if self.set_exec_times > self.set_exec_times_to_discard {
            avg_set_exec_time_ns = self.cumul_set_exec_time / ((self.set_exec_times - self.set_exec_times_to_discard) as f64);
        }
        let perf = MerklePerfStats { avg_set_exec_time_ns: avg_set_exec_time_ns };
        Ok(MerkleStorageStats { rocksdb_stats: db_stats, map_stats: self.map_stats, perf_stats: perf })
    }
}

#[cfg(test)]
#[allow(unused_must_use)]
mod tests {
    use std::{env, fs};
    use std::path::{Path, PathBuf};

    use rocksdb::{DB, Options};

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
        let path = Path::new(out_dir.as_str())
            .join(Path::new(dir_name))
            .to_path_buf();
        path
    }

    fn get_db_name(db_name: &str) -> PathBuf {
        out_dir_path(db_name)
    }

    fn get_db(db_name: &str, cache: &Cache) -> DB { open_db(get_db_name(db_name), &cache) }

    fn get_storage(dn_name: &str, cache: &Cache) -> MerkleStorage { MerkleStorage::new(Arc::new(get_db(dn_name, &cache))) }

    fn clean_db(db_name: &str) {
        let _ = DB::destroy(&Options::default(), get_db_name(db_name));
        let _ = fs::remove_dir_all(get_db_name(db_name));
    }

    #[test]
    fn test_tree_hash() {
        let cache = Cache::new_lru_cache(32 * 1024 * 1024).unwrap();
        let mut storage = get_storage("ms_test_tree_hash", &cache);
        storage.set(&vec!["a".to_string(), "foo".to_string()], &vec![97, 98, 99]); // abc
        storage.set(&vec!["b".to_string(), "boo".to_string()], &vec![97, 98]);
        storage.set(&vec!["a".to_string(), "aaa".to_string()], &vec![97, 98, 99, 100]);
        storage.set(&vec!["x".to_string()], &vec![97]);
        storage.set(&vec!["one".to_string(), "two".to_string(), "three".to_string()], &vec![97]);
        let tree = storage.current_stage_tree.clone().unwrap().clone();

        let hash = storage.hash_tree(&tree).unwrap();

        assert_eq!([0xDB, 0xAE, 0xD7, 0xB6], hash[0..4]);
    }

    #[test]
    fn test_commit_hash() {
        let cache = Cache::new_lru_cache(32 * 1024 * 1024).unwrap();
        let mut storage = get_storage("ms_test_commit_hash", &cache);
        storage.set(&vec!["a".to_string()], &vec![97, 98, 99]);

        let commit = storage.commit(
            0, "Tezos".to_string(), "Genesis".to_string());

        assert_eq!([0xCF, 0x95, 0x18, 0x33], commit.unwrap()[0..4]);

        storage.set(&vec!["data".to_string(), "x".to_string()], &vec![97]);
        let commit = storage.commit(
            0, "Tezos".to_string(), "".to_string());

        assert_eq!([0xCA, 0x7B, 0xC7, 0x02], commit.unwrap()[0..4]);
        // full irmin hash: ca7bc7022ffbd35acc97f7defb00c486bb7f4d19a2d62790d5949775eb74f3c8
    }

    #[test]
    fn test_multiple_commit_hash() {
        let cache = Cache::new_lru_cache(32 * 1024 * 1024).unwrap();
        let mut storage = get_storage("ms_test_multiple_commit_hash", &cache);
        let _commit = storage.commit(
            0, "Tezos".to_string(), "Genesis".to_string());

        storage.set(&vec!["data".to_string(), "a".to_string(), "x".to_string()], &vec![97]);
        storage.copy(&vec!["data".to_string(), "a".to_string()], &vec!["data".to_string(), "b".to_string()]);
        storage.delete(&vec!["data".to_string(), "b".to_string(), "x".to_string()]);
        let commit = storage.commit(
            0, "Tezos".to_string(), "".to_string());

        assert_eq!([0x9B, 0xB0, 0x0D, 0x6E], commit.unwrap()[0..4]);
    }

    #[test]
    fn get_test() {
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
            storage.set(key_abc, &vec![1u8, 2u8]);
            storage.set(key_abx, &vec![3u8]);
       //     assert_eq!(storage.get(&key_abc).unwrap(), vec![1u8, 2u8]);
        //    assert_eq!(storage.get(&key_abx).unwrap(), vec![3u8]);
            commit1 = storage.commit(0, "".to_string(), "".to_string()).unwrap();

            storage.set(key_az, &vec![4u8]);
            storage.set(key_abx, &vec![5u8]);
            storage.set(key_d, &vec![6u8]);
            storage.set(key_eab, &vec![7u8]);
         //   assert_eq!(storage.get(key_abx).unwrap(), vec![5u8]);
            commit2 = storage.commit(0, "".to_string(), "".to_string()).unwrap();
        }

        let cache = Cache::new_lru_cache(32 * 1024 * 1024).unwrap();
        let storage = get_storage(db_name, &cache);
        assert_eq!(storage.get_history(&commit1, key_abc).unwrap(), vec![1u8, 2u8]);
        assert_eq!(storage.get_history(&commit1, key_abx).unwrap(), vec![3u8]);
        assert_eq!(storage.get_history(&commit2, key_abx).unwrap(), vec![5u8]);
        assert_eq!(storage.get_history(&commit2, key_az).unwrap(), vec![4u8]);
        assert_eq!(storage.get_history(&commit2, key_d).unwrap(), vec![6u8]);
        assert_eq!(storage.get_history(&commit2, key_eab).unwrap(), vec![7u8]);
    }

    #[test]
    fn test_copy() {
        let db_name = "ms_test_copy";
        clean_db(db_name);

        let cache = Cache::new_lru_cache(32 * 1024 * 1024).unwrap();
        let mut storage = get_storage(db_name, &cache);
        let key_abc: &ContextKey = &vec!["a".to_string(), "b".to_string(), "c".to_string()];
        storage.set(key_abc, &vec![1 as u8]);
        storage.copy(&vec!["a".to_string()], &vec!["z".to_string()]);

        assert_eq!(
            vec![1 as u8],
            storage.get(&vec!["z".to_string(), "b".to_string(), "c".to_string()]).unwrap());
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
        storage.set(key_abc, &vec![2 as u8]);
        storage.set(key_abx, &vec![3 as u8]);
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
        storage.set(key_abc, &vec![2 as u8]);
        let commit1 = storage.commit(0, "".to_string(), "".to_string()).unwrap();
        storage.delete(key_abc);
        let _commit2 = storage.commit(0, "".to_string(), "".to_string()).unwrap();

        assert_eq!(vec![2 as u8], storage.get_history(&commit1, &key_abc).unwrap());
    }

    #[test]
    fn test_delete_in_separate_commit() {
        let db_name = "ms_test_delete_in_separate_commit";
        clean_db(db_name);

        let cache = Cache::new_lru_cache(32 * 1024 * 1024).unwrap();
        let mut storage = get_storage(db_name, &cache);
        let key_abc: &ContextKey = &vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let key_abx: &ContextKey = &vec!["a".to_string(), "b".to_string(), "x".to_string()];
        storage.set(key_abc, &vec![2 as u8]).unwrap();
        storage.set(key_abx, &vec![3 as u8]).unwrap();
        storage.commit(0, "".to_string(), "".to_string()).unwrap();

        storage.delete(key_abx);
        let commit2 = storage.commit(
            0, "".to_string(), "".to_string()).unwrap();

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
        { clean_db(db_name); }

        let key_abc: &ContextKey = &vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let commit1;
        {
            let cache = Cache::new_lru_cache(32 * 1024 * 1024).unwrap();
            let mut storage = get_storage(db_name, &cache);
            let key_abx: &ContextKey = &vec!["a".to_string(), "b".to_string(), "x".to_string()];
            storage.set(key_abc, &vec![2 as u8]).unwrap();
            storage.set(key_abx, &vec![3 as u8]).unwrap();
            commit1 = storage.commit(0, "".to_string(), "".to_string()).unwrap();
        }

        let cache = Cache::new_lru_cache(32 * 1024 * 1024).unwrap();
        let storage = get_storage(db_name, &cache);
        assert_eq!(vec![2 as u8], storage.get_history(&commit1, &key_abc).unwrap());
    }

    #[test]
    fn test_get_errors() {
        let db_name = "ms_test_get_errors";
        { clean_db(db_name); }

        let cache = Cache::new_lru_cache(32 * 1024 * 1024).unwrap();
        let mut storage = get_storage(db_name, &cache);

        let res = storage.get(&vec![]);
        assert!(if let MerkleError::KeyEmpty = res.err().unwrap() { true } else { false });

        let res = storage.get(&vec!["a".to_string()]);
        assert!(if let MerkleError::ValueNotFound { .. } = res.err().unwrap() { true } else { false });
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

        let db = DB::open_for_read_only(
            &Options::default(), get_db_name(db_name), true).unwrap();
        let mut storage = MerkleStorage::new(Arc::new(db));
        storage.set(&vec!["a".to_string()], &vec![1u8]);
        let res = storage.commit(
            0, "".to_string(), "".to_string());

        assert!(if let MerkleError::DBError { .. } = res.err().unwrap() { true } else { false });
    }

    // Test getting entire tree in string format for JSON RPC
    #[test]
    fn test_get_context_tree_by_prefix() {
        let db_name = "ms_test_get_context_tree_by_prefix";
        { clean_db(db_name); }

        let all_json = "{\"adata\":{\"b\":{\"x\":{\"y\":\"090a\"}}},\
                        \"data\":{\"a\":{\"x\":{\"y\":\"0506\"}},\
                        \"b\":{\"x\":{\"y\":\"0708\"}},\"c\":\"0102\"}}";
        let data_json = "{\
                        \"a\":{\"x\":{\"y\":\"0506\"}},\
                        \"b\":{\"x\":{\"y\":\"0708\"}},\"c\":\"0102\"}";

        let cache = Cache::new_lru_cache(32 * 1024 * 1024).unwrap();
        let mut storage = get_storage(db_name, &cache);
        let _commit = storage.commit(0, "Tezos".to_string(), "Genesis".to_string());

        storage.set(&vec!["data".to_string(), "a".to_string(), "x".to_string()], &vec![3, 4]);
        storage.set(&vec!["data".to_string(), "a".to_string()], &vec![1, 2]);
        storage.set(&vec!["data".to_string(), "a".to_string(), "x".to_string(), "y".to_string()], &vec![5, 6]);
        storage.set(&vec!["data".to_string(), "b".to_string(), "x".to_string(), "y".to_string()], &vec![7, 8]);
        storage.set(&vec!["data".to_string(), "c".to_string()], &vec![1, 2]);
        storage.set(&vec!["adata".to_string(), "b".to_string(), "x".to_string(), "y".to_string()], &vec![9, 10]);
        //data-a[1,2]
        //data-a-x[3,4]
        //data-a-x-y[5,6]
        //data-b-x-y[7,8]
        //data-c[1,2]
        //adata-b-x-y[9,10]
        let commit = storage.commit(0, "Tezos".to_string(), "Genesis".to_string());
        let rv_all = storage.get_context_tree_by_prefix(&commit.as_ref().unwrap(), &vec![]).unwrap();
        let rv_data = storage.get_context_tree_by_prefix(&commit.as_ref().unwrap(), &vec!["data".to_string()]).unwrap();
        assert_eq!(all_json, serde_json::to_string(&rv_all).unwrap());
        assert_eq!(data_json, serde_json::to_string(&rv_data).unwrap());
    }
}
