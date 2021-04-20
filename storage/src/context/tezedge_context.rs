// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::convert::TryFrom;
use std::convert::TryInto;
use std::sync::{Arc, Mutex};

use failure::Error;

use crypto::hash::{BlockHash, ContextHash, FromBytesError};

use crate::context::actions::context_action_storage::ContextAction;
use crate::context::actions::{get_new_tree_hash, get_tree_id};
use crate::context::merkle::hash::EntryHash;
use crate::context::merkle::merkle_storage::{MerkleError, MerkleStorage};
use crate::context::merkle::merkle_storage_stats::MerkleStoragePerfReport;
use crate::context::{ContextApi, ContextError, ContextKey, ContextValue, StringTreeEntry, TreeId};
use crate::{BlockStorage, BlockStorageReader, StorageError};

impl ContextApi for TezedgeContext {
    fn set(
        &mut self,
        _context_hash: &Option<ContextHash>,
        new_tree_id: TreeId,
        key: &ContextKey,
        value: ContextValue,
    ) -> Result<(), ContextError> {
        let mut merkle = self.merkle.lock()?;
        merkle.set(new_tree_id, key, value)?;

        Ok(())
    }

    fn checkout(&self, context_hash: &ContextHash) -> Result<(), ContextError> {
        let context_hash_arr: EntryHash = context_hash.as_ref().as_slice().try_into()?;
        let mut merkle = self.merkle.lock()?;
        merkle.checkout(&context_hash_arr)?;

        Ok(())
    }

    fn commit(
        &mut self,
        block_hash: &BlockHash,
        parent_context_hash: &Option<ContextHash>,
        author: String,
        message: String,
        date: i64,
    ) -> Result<ContextHash, ContextError> {
        let mut merkle = self.merkle.lock()?;

        let date: u64 = date.try_into()?;
        let commit_hash = merkle.commit(date, author, message)?;
        let commit_hash = ContextHash::try_from(&commit_hash[..])?;

        self.associate_block_and_context_hash(block_hash, &commit_hash, parent_context_hash)?;

        Ok(commit_hash)
    }

    fn delete_to_diff(
        &self,
        _context_hash: &Option<ContextHash>,
        new_tree_id: TreeId,
        key_prefix_to_delete: &ContextKey,
    ) -> Result<(), ContextError> {
        let mut merkle = self.merkle.lock()?;
        merkle.delete(new_tree_id, key_prefix_to_delete)?;
        Ok(())
    }

    fn remove_recursively_to_diff(
        &self,
        _context_hash: &Option<ContextHash>,
        new_tree_id: TreeId,
        key_prefix_to_remove: &ContextKey,
    ) -> Result<(), ContextError> {
        let mut merkle = self.merkle.lock()?;
        merkle.delete(new_tree_id, key_prefix_to_remove)?;
        Ok(())
    }

    fn copy_to_diff(
        &self,
        _context_hash: &Option<ContextHash>,
        new_tree_id: TreeId,
        from_key: &ContextKey,
        to_key: &ContextKey,
    ) -> Result<(), ContextError> {
        let mut merkle = self.merkle.lock()?;
        merkle.copy(new_tree_id, from_key, to_key)?;
        Ok(())
    }

    fn get_key(&self, key: &ContextKey) -> Result<ContextValue, ContextError> {
        let mut merkle = self.merkle.lock()?;
        let val = merkle.get(key)?;
        Ok(val)
    }

    fn mem(&self, key: &ContextKey) -> Result<bool, ContextError> {
        let mut merkle = self.merkle.lock()?;
        let val = merkle.mem(key)?;
        Ok(val)
    }

    fn dirmem(&self, key: &ContextKey) -> Result<bool, ContextError> {
        let mut merkle = self.merkle.lock()?;
        let val = merkle.dirmem(key)?;
        Ok(val)
    }

    fn get_key_from_history(
        &self,
        context_hash: &ContextHash,
        key: &ContextKey,
    ) -> Result<Option<ContextValue>, ContextError> {
        let context_hash_arr: EntryHash = context_hash.as_ref().as_slice().try_into()?;
        let mut merkle = self.merkle.lock()?;
        match merkle.get_history(&context_hash_arr, key) {
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
        let mut merkle = self.merkle.lock()?;
        merkle
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
        let mut merkle = self.merkle.lock()?;
        merkle
            .get_context_tree_by_prefix(&context_hash_arr, prefix, depth)
            .map_err(ContextError::from)
    }

    fn get_last_commit_hash(&self) -> Result<Option<Vec<u8>>, ContextError> {
        let merkle = self.merkle.lock()?;
        Ok(merkle.get_last_commit_hash().map(|x| x.to_vec()))
    }

    fn get_merkle_stats(&self) -> Result<MerkleStoragePerfReport, ContextError> {
        let merkle = self.merkle.lock()?;
        Ok(merkle.get_merkle_stats()?)
    }

    fn is_committed(&self, context_hash: &ContextHash) -> Result<bool, ContextError> {
        match &self.block_storage {
            Some(block_storage) => block_storage
                .contains_context_hash(context_hash)
                .map_err(|e| ContextError::StorageError { error: e }),
            None => Err(ContextError::CommitStatusCheckFailure {}),
        }
    }

    fn set_merkle_root(&mut self, tree_id: TreeId) -> Result<(), ContextError> {
        let mut merkle = self.merkle.lock()?;
        merkle
            .working_tree_checkout(tree_id)
            .map_err(ContextError::from)
    }

    fn get_merkle_root(&mut self) -> Result<EntryHash, ContextError> {
        let merkle = self.merkle.lock()?;
        merkle
            .get_working_tree_root_hash()
            .map_err(ContextError::from)
    }

    fn block_applied(&self) -> Result<(), ContextError> {
        let mut merkle = self.merkle.lock()?;
        Ok(merkle.block_applied()?)
    }

    fn cycle_started(&self) -> Result<(), ContextError> {
        let mut merkle = self.merkle.lock()?;
        Ok(merkle.start_new_cycle()?)
    }

    fn get_memory_usage(&self) -> Result<usize, ContextError> {
        let merkle = self.merkle.lock()?;
        Ok(merkle.get_memory_usage()?)
    }

    fn perform_context_action(&mut self, action: ContextAction) -> Result<(), Error> {
        let new_tree_id = get_tree_id(&action);
        let new_tree_hash = get_new_tree_hash(&action)?;

        if let Some(tree_id) = new_tree_id {
            self.set_merkle_root(tree_id)?;
        }

        match action {
            ContextAction::Get { key, .. } => {
                self.get_key(&key)?;
            }
            ContextAction::Mem { key, .. } => {
                self.mem(&key)?;
            }
            ContextAction::DirMem { key, .. } => {
                self.dirmem(&key)?;
            }
            ContextAction::Set {
                key,
                value,
                new_tree_id,
                context_hash,
                ..
            } => {
                let context_hash = try_from_untyped_option(context_hash)?;
                self.set(&context_hash, new_tree_id, &key, value)?;
            }
            ContextAction::Copy {
                to_key: key,
                from_key,
                new_tree_id,
                context_hash,
                ..
            } => {
                let context_hash = try_from_untyped_option(context_hash)?;
                self.copy_to_diff(&context_hash, new_tree_id, &from_key, &key)?;
            }
            ContextAction::Delete {
                key,
                new_tree_id,
                context_hash,
                ..
            } => {
                let context_hash = try_from_untyped_option(context_hash)?;
                self.delete_to_diff(&context_hash, new_tree_id, &key)?;
            }
            ContextAction::RemoveRecursively {
                key,
                new_tree_id,
                context_hash,
                ..
            } => {
                let context_hash = try_from_untyped_option(context_hash)?;
                self.remove_recursively_to_diff(&context_hash, new_tree_id, &key)?;
            }
            ContextAction::Commit {
                parent_context_hash,
                new_context_hash,
                block_hash: Some(block_hash),
                author,
                message,
                date,
                ..
            } => {
                let parent_context_hash = try_from_untyped_option(parent_context_hash)?;
                // TODO: not necessery clone, remove here when disconnect from block_storage
                let block_hash = BlockHash::try_from(block_hash)?;
                let hash = self.commit(&block_hash, &parent_context_hash, author, message, date)?;
                let new_context_hash = ContextHash::try_from(new_context_hash)?;
                assert_eq!(
                    &hash,
                    &new_context_hash,
                    "Invalid context_hash for block: {}, expected: {}, but was: {}",
                    block_hash.to_base58_check(),
                    new_context_hash.to_base58_check(),
                    hash.to_base58_check(),
                );
            }

            ContextAction::Checkout { context_hash, .. } => {
                self.checkout(&ContextHash::try_from(context_hash)?)?;
            }

            ContextAction::Commit { .. } => (), // Ignored (no block_hash)

            ContextAction::Fold { .. } => (), // Ignored

            ContextAction::Shutdown => (), // Ignored
        };

        if let Some(post_hash) = new_tree_hash {
            assert_eq!(
                self.get_merkle_root()?,
                post_hash,
                "Invalid tree_hash context: {:?}, post_hash: {:?}, tree_id: {:? }",
                self.get_merkle_root()?,
                post_hash,
                new_tree_id,
            );
        }

        Ok(())
    }
}

fn try_from_untyped_option<H>(h: Option<Vec<u8>>) -> Result<Option<H>, FromBytesError>
where
    H: TryFrom<Vec<u8>, Error = FromBytesError>,
{
    h.map(H::try_from).map_or(Ok(None), |r| r.map(Some))
}

// context implementation using merkle-tree-like storage
#[derive(Clone)]
pub struct TezedgeContext {
    block_storage: Option<BlockStorage>,
    merkle: Arc<Mutex<MerkleStorage>>,
}

impl TezedgeContext {
    pub fn new(block_storage: Option<BlockStorage>, merkle: Arc<Mutex<MerkleStorage>>) -> Self {
        TezedgeContext {
            block_storage,
            merkle,
        }
    }

    fn associate_block_and_context_hash(
        &self,
        block_hash: &BlockHash,
        commit_hash: &ContextHash,
        parent_context_hash: &Option<ContextHash>,
    ) -> Result<(), ContextError> {
        if let Some(storage) = &self.block_storage {
            if let Err(e) = storage.assign_to_context(block_hash, &commit_hash) {
                match e {
                    StorageError::MissingKey => {
                        // TODO: is this needed? check it when removing assign_to_context
                        if parent_context_hash.is_some() {
                            return Err(ContextError::ContextHashAssignError {
                                block_hash: block_hash.to_base58_check(),
                                context_hash: commit_hash.to_base58_check(),
                                error: e,
                            });
                        } else {
                            // TODO: do correctly assignement on one place, or remove this assignemnt - it is not needed
                            // if parent_context_hash is empty, means it is commit_genesis, and block is not already stored, thats ok
                            // but we need to storage assignment elsewhere
                        }
                    }
                    _ => {
                        return Err(ContextError::ContextHashAssignError {
                            block_hash: block_hash.to_base58_check(),
                            context_hash: commit_hash.to_base58_check(),
                            error: e,
                        });
                    }
                };
            }
        }
        Ok(())
    }
}
