// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    cell::{Ref, RefCell},
    convert::TryInto,
};
use std::{convert::TryFrom, rc::Rc};

use crypto::hash::ContextHash;

use crate::working_tree::working_tree_stats::MerkleStoragePerfReport;
use crate::{
    hash::EntryHash,
    working_tree::{Commit, Entry, Tree},
};
use crate::{
    working_tree::working_tree::{MerkleError, WorkingTree},
    IndexApi,
};
use crate::{
    ContextError, ContextKey, ContextValue, ProtocolContextApi, ShellContextApi, StringTreeEntry,
    TreeId,
};

use crate::{working_tree::working_tree::StagedCache, ContextKeyValueStore};

#[derive(Clone)]
pub struct TezedgeIndex {
    pub repository: Rc<RefCell<ContextKeyValueStore>>,
}

impl TezedgeIndex {
    pub fn new(repository: Rc<RefCell<ContextKeyValueStore>>) -> Self {
        Self { repository }
    }
}

impl IndexApi<TezedgeContext> for TezedgeIndex {
    fn exists(&self, context_hash: &ContextHash) -> Result<bool, ContextError> {
        let context_hash_arr: EntryHash = context_hash.as_ref().as_slice().try_into()?;
        if let Some(Entry::Commit(_)) = db_get_entry(self.repository.borrow(), &context_hash_arr)? {
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn checkout(&self, context_hash: &ContextHash) -> Result<Option<TezedgeContext>, ContextError> {
        let context_hash_arr: EntryHash = context_hash.as_ref().as_slice().try_into()?;

        if let Some(commit) = db_get_commit(self.repository.borrow(), &context_hash_arr)? {
            if let Some(tree) = db_get_tree(self.repository.borrow(), &commit.root_hash)? {
                let staged_cache = Rc::new(RefCell::new(StagedCache::new(self.clone())));
                let tree = WorkingTree::new_with_tree(staged_cache, tree);

                Ok(Some(TezedgeContext::new(
                    self.clone(),
                    Some(context_hash_arr),
                    Some(Rc::new(tree)),
                )))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }
}

// context implementation using merkle-tree-like storage
#[derive(Clone)]
pub struct TezedgeContext {
    pub index: TezedgeIndex,
    pub parent_commit_hash: Option<EntryHash>, // TODO: Rc instead?
    pub tree_id: TreeId,
    tree_id_generator: Rc<RefCell<TreeIdGenerator>>,
    pub tree: Rc<WorkingTree>,
}

impl ProtocolContextApi for TezedgeContext {
    fn set(&self, key: &ContextKey, value: ContextValue) -> Result<Self, ContextError> {
        let tree = self.tree.set(key, value)?;

        Ok(self.with_tree(tree))
    }

    fn delete(&self, key_prefix_to_delete: &ContextKey) -> Result<Self, ContextError> {
        let tree = self.tree.delete(key_prefix_to_delete)?;

        Ok(self.with_tree(tree))
    }

    fn copy(
        &self,
        from_key: &ContextKey,
        to_key: &ContextKey,
    ) -> Result<Option<Self>, ContextError> {
        if let Some(tree) = self.tree.copy(from_key, to_key)? {
            Ok(Some(self.with_tree(tree)))
        } else {
            Ok(None)
        }
    }

    fn get(&self, key: &ContextKey) -> Result<ContextValue, ContextError> {
        let val = self.tree.get(key)?;
        Ok(val)
    }

    fn mem(&self, key: &ContextKey) -> Result<bool, ContextError> {
        let val = self.tree.mem(key)?;
        Ok(val)
    }

    fn dirmem(&self, key: &ContextKey) -> Result<bool, ContextError> {
        let val = self.tree.dirmem(key)?;
        Ok(val)
    }

    fn get_merkle_root(&self) -> Result<EntryHash, ContextError> {
        Ok(self.tree.get_staged_root_hash()?)
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
        let (commit_hash, batch) =
            self.tree
                .prepare_commit(date, author, message, self.parent_commit_hash)?;
        self.index.repository.borrow_mut().write_batch(batch)?;
        let commit_hash = ContextHash::try_from(&commit_hash[..])?;

        Ok(commit_hash)
    }

    fn hash(
        &self,
        author: String,
        message: String,
        date: i64,
    ) -> Result<ContextHash, ContextError> {
        // FIXME: does more work than needed, just calculate the hash, no batch
        let date: u64 = date.try_into()?;
        let (commit_hash, _batch) =
            self.tree
                .prepare_commit(date, author, message, self.parent_commit_hash)?;
        let commit_hash = ContextHash::try_from(&commit_hash[..])?;

        Ok(commit_hash)
    }

    fn get_key_from_history(
        &self,
        context_hash: &ContextHash,
        key: &ContextKey,
    ) -> Result<Option<ContextValue>, ContextError> {
        let context_hash_arr: EntryHash = context_hash.as_ref().as_slice().try_into()?;
        match self.tree.get_history(&context_hash_arr, key) {
            Err(MerkleError::ValueNotFound { key: _ }) => Ok(None),
            Err(MerkleError::EntryNotFound { hash: _ }) => {
                Err(ContextError::UnknownContextHashError {
                    context_hash: context_hash.to_base58_check(),
                })
            }
            Err(err) => Err(ContextError::MerkleStorageError { error: err }),
            Ok(val) => Ok(Some(val)),
        }
    }

    fn get_key_values_by_prefix(
        &self,
        context_hash: &ContextHash,
        prefix: &ContextKey,
    ) -> Result<Option<Vec<(ContextKey, ContextValue)>>, ContextError> {
        let context_hash_arr: EntryHash = context_hash.as_ref().as_slice().try_into()?;
        self.tree
            .get_key_values_by_prefix(&context_hash_arr, prefix)
            .map_err(ContextError::from)
    }

    fn get_context_tree_by_prefix(
        &self,
        context_hash: &ContextHash,
        prefix: &ContextKey,
        depth: Option<usize>,
    ) -> Result<StringTreeEntry, ContextError> {
        let context_hash_arr: EntryHash = context_hash.as_ref().as_slice().try_into()?;
        self.tree
            .get_context_tree_by_prefix(&context_hash_arr, prefix, depth)
            .map_err(ContextError::from)
    }

    fn get_last_commit_hash(&self) -> Result<Option<Vec<u8>>, ContextError> {
        Ok(self.parent_commit_hash.map(|x| x.to_vec()))
    }

    fn get_merkle_stats(&self) -> Result<MerkleStoragePerfReport, ContextError> {
        Ok(self.tree.get_merkle_stats()?)
    }

    fn block_applied(&mut self, last_commit_hash: ContextHash) -> Result<(), ContextError> {
        let commit_hash_arr: EntryHash = last_commit_hash.as_ref().as_slice().try_into()?;
        Ok(self
            .index
            .repository
            .borrow_mut()
            .block_applied(commit_hash_arr)?)
    }

    fn cycle_started(&mut self) -> Result<(), ContextError> {
        Ok(self.index.repository.borrow_mut().new_cycle_started()?)
    }

    fn get_memory_usage(&self) -> Result<usize, ContextError> {
        Ok(self.index.repository.borrow().total_get_mem_usage()?)
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
        parent_commit_hash: Option<EntryHash>,
        tree: Option<Rc<WorkingTree>>,
    ) -> Self {
        let tree = if let Some(tree) = tree {
            tree
        } else {
            Rc::new(WorkingTree::new(Rc::new(RefCell::new(StagedCache::new(
                index.clone(),
            )))))
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
}

fn db_get_entry(
    db: Ref<ContextKeyValueStore>,
    hash: &EntryHash,
) -> Result<Option<Entry>, ContextError> {
    match db.get(hash)? {
        None => Ok(None),
        Some(entry_bytes) => Ok(Some(bincode::deserialize(&entry_bytes)?)),
    }
}

fn db_get_commit(
    db: Ref<ContextKeyValueStore>,
    hash: &EntryHash,
) -> Result<Option<Commit>, ContextError> {
    match db_get_entry(db, hash)? {
        Some(Entry::Commit(commit)) => Ok(Some(commit)),
        Some(Entry::Tree(_)) => Err(ContextError::FoundUnexpectedStructure {
            sought: "commit".to_string(),
            found: "tree".to_string(),
        }),
        Some(Entry::Blob(_)) => Err(ContextError::FoundUnexpectedStructure {
            sought: "commit".to_string(),
            found: "blob".to_string(),
        }),
        None => Ok(None),
    }
}

fn db_get_tree(
    db: Ref<ContextKeyValueStore>,
    hash: &EntryHash,
) -> Result<Option<Tree>, ContextError> {
    match db_get_entry(db, hash)? {
        Some(Entry::Tree(tree)) => Ok(Some(tree)),
        Some(Entry::Blob(_)) => Err(ContextError::FoundUnexpectedStructure {
            sought: "tree".to_string(),
            found: "blob".to_string(),
        }),
        Some(Entry::Commit { .. }) => Err(ContextError::FoundUnexpectedStructure {
            sought: "tree".to_string(),
            found: "commit".to_string(),
        }),
        None => Ok(None),
    }
}
