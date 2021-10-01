// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! This module covers helper functionality with managing current head attirbute for chain managers

use std::cmp::Ordering;
use std::collections::HashSet;
use std::convert::TryInto;
use std::fmt;
use std::sync::Arc;

use getset::Getters;

use crypto::hash::ChainId;
use storage::chain_meta_storage::ChainMetaStorageReader;
use storage::predecessor_storage::PredecessorSearch;
use storage::{
    block_storage, BlockHeaderWithHash, BlockStorage, BlockStorageReader, ChainMetaStorage,
};
use storage::{PersistentStorage, StorageError};
use tezos_messages::p2p::encoding::block_header::Level;
use tezos_messages::Head;

use crate::mempool::CurrentMempoolStateStorageRef;
use crate::state::StateError;
use crate::validation;

pub enum HeadResult {
    BranchSwitch,
    HeadIncrement,
}

impl fmt::Display for HeadResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            HeadResult::BranchSwitch => write!(f, "BranchSwitch"),
            HeadResult::HeadIncrement => write!(f, "HeadIncrement"),
        }
    }
}

#[derive(Clone, Getters)]
pub struct HeadState {
    ///persistent chain metadata storage
    chain_meta_storage: ChainMetaStorage,

    ///persistent block storage
    block_storage: BlockStorage,

    /// Current head information (required, at least genesis should be here)
    current_head: Head,

    #[get = "pub(crate)"]
    checkpoint: Head,

    alternate_heads: HashSet<Head>,

    chain_id: Arc<ChainId>,
}

impl AsRef<Head> for HeadState {
    fn as_ref(&self) -> &Head {
        &self.current_head
    }
}

impl HeadState {
    pub fn new(
        persistent_storage: &PersistentStorage,
        current_head: Head,
        checkpoint: Head,
        alternate_heads: HashSet<Head>,
        chain_id: Arc<ChainId>,
    ) -> Self {
        HeadState {
            chain_meta_storage: ChainMetaStorage::new(persistent_storage),
            block_storage: BlockStorage::new(persistent_storage),
            current_head,
            checkpoint,
            alternate_heads,
            chain_id,
        }
    }

    /// Resolve if new applied block can be set as new current head.
    /// Original algorithm is in [chain_validator][on_request], where just fitness is checked.
    /// Returns:
    /// - None, if head was not updated, means was ignored
    /// - Some(new_head, head_result)
    pub fn try_update_new_current_head(
        &mut self,
        potential_new_head: &BlockHeaderWithHash,
        current_mempool_state: &CurrentMempoolStateStorageRef,
        last_allowed_fork_level: i32,
    ) -> Result<Option<(Head, HeadResult)>, StateError> {
        // check if we can update head
        // get fitness from mempool, if not, than use current_head.fitness
        let mempool_state = current_mempool_state.read()?;
        let current_context_fitness = {
            if let Some(Some(fitness)) = mempool_state
                .prevalidator()
                .map(|p| p.context_fitness.as_ref())
            {
                fitness
            } else {
                self.current_head.fitness()
            }
        };
        // need to check against current_head, if not accepted, just ignore potential head
        if !validation::can_update_current_head(
            potential_new_head,
            &self.current_head,
            current_context_fitness,
        ) {
            // just ignore
            return Ok(None);
        }

        // this will be new head
        let head = Head::new(
            potential_new_head.hash.clone(),
            potential_new_head.header.level(),
            potential_new_head.header.fitness().clone(),
        );

        if last_allowed_fork_level > *self.checkpoint.level() {
            if let Some(checkpoint_block) = self
                .block_storage
                .get_by_level(potential_new_head, last_allowed_fork_level)?
            {
                let head_representation = Head::new(
                    checkpoint_block.hash,
                    checkpoint_block.header.level(),
                    checkpoint_block.header.fitness().to_vec(),
                );

                // new checkpoint to db
                self.chain_meta_storage
                    .set_checkpoint(&self.chain_id, head_representation.clone())?;

                // new checkpoint to in-memory
                self.checkpoint = head_representation;
            }
        }

        let current_head = self.current_head.clone();

        // we need to check, if previous head is predecessor of new_head (for later use)
        let head_result = if current_head
            .block_hash()
            .eq(potential_new_head.header.predecessor())
        {
            self.trim_alternate_heads();
            HeadResult::HeadIncrement
        } else {
            // if previous head is not predecesor of new head, means it could be new branch
            match self.get_alternate_head_ancestor(&head)? {
                Some(alternate_head) => {
                    self.alternate_heads.remove(&alternate_head);
                }
                None => self.trim_alternate_heads(),
            }
            self.alternate_heads.insert(current_head);

            HeadResult::BranchSwitch
        };

        // set new head to db
        self.chain_meta_storage
            .set_current_head(&self.chain_id, head.clone())?;

        // set new head to in-memory
        self.current_head = head.clone();

        Ok(Some((head, head_result)))
    }

    /// Tries to load last known current head from database
    /// Returns Some(previous) if changed, else None
    pub(crate) fn reload_current_head_state(&mut self) -> Result<Option<Head>, StorageError> {
        match self.chain_meta_storage.get_current_head(&self.chain_id)? {
            Some(head) => {
                if !head.block_hash().eq(self.current_head.block_hash()) {
                    Ok(Some(std::mem::replace(&mut self.current_head, head)))
                } else {
                    Ok(None)
                }
            }
            None => Ok(None),
        }
    }

    fn get_alternate_head_ancestor(&self, new_head: &Head) -> Result<Option<Head>, StorageError> {
        let block_storage = self.block_storage.clone();

        for alternate_head in &self.alternate_heads {
            if is_ancestor(new_head, alternate_head, &block_storage)? {
                return Ok(Some(alternate_head.clone()));
            }
        }
        Ok(None)
    }

    fn trim_alternate_heads(&mut self) {
        let checkpoint = self.checkpoint.clone();
        let block_storage = self.block_storage.clone();
        self.alternate_heads
            .retain(|ah| match is_ancestor(&checkpoint, ah, &block_storage) {
                Ok(is_ancestor) => is_ancestor,
                _ => false,
            });
    }

    // TODO: maybe move this to the storage level?
}

fn is_ancestor(
    block: &Head,
    ancestor: &Head,
    block_storage: &BlockStorage,
) -> Result<bool, StorageError> {
    match ancestor.level().cmp(block.level()) {
        Ordering::Greater => Ok(false),
        Ordering::Equal => Ok(block.block_hash().eq(ancestor.block_hash())),
        _ => {
            let distance = block.level() - ancestor.level();
            if distance < 0 {
                return Err(StorageError::NegativeDistanceError);
            }
            match block_storage
                .find_block_at_distance(block.block_hash().clone(), distance as u32)?
            {
                Some(found_block_hash) => Ok(found_block_hash.eq(ancestor.block_hash())),
                None => Ok(false),
            }
        }
    }
}

/// This struct holds info best known remote "current" head
pub struct RemoteBestKnownCurrentHead {
    /// Remote current head. This represents info about
    /// the current branch with the highest level received from network.
    remote: Option<Head>,
}

impl AsRef<Option<Head>> for RemoteBestKnownCurrentHead {
    fn as_ref(&self) -> &Option<Head> {
        &self.remote
    }
}

impl RemoteBestKnownCurrentHead {
    pub fn new() -> Self {
        RemoteBestKnownCurrentHead { remote: None }
    }

    fn need_update_remote_level(&self, new_remote_level: i32) -> bool {
        match &self.remote {
            None => true,
            Some(current_remote_head) => new_remote_level > *current_remote_head.level(),
        }
    }

    pub fn update_remote_head(&mut self, block_header: &BlockHeaderWithHash) {
        // TODO: maybe fitness check?
        if self.need_update_remote_level(block_header.header.level()) {
            self.remote = Some(Head::new(
                block_header.hash.clone(),
                block_header.header.level(),
                block_header.header.fitness().to_vec(),
            ));
        }
    }
}

pub fn has_any_higher_than(
    local_current_head: impl AsRef<Head>,
    remote_current_head: impl AsRef<Option<Head>>,
    level_to_check: Level,
) -> bool {
    // check remote head
    // TODO: maybe fitness check?
    if let Some(remote_head) = remote_current_head.as_ref() {
        if remote_head.level() > &level_to_check {
            return true;
        }
    }

    // check local head
    // TODO: maybe fitness check?
    if local_current_head.as_ref().level() > &level_to_check {
        return true;
    }

    false
}
