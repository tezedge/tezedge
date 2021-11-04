// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! This module covers helper functionality with managing current head attirbute for chain managers

use std::fmt;
use std::sync::Arc;

use crypto::hash::ChainId;
use storage::chain_meta_storage::ChainMetaStorageReader;
use storage::{BlockHeaderWithHash, ChainMetaStorage};
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

pub struct HeadState {
    ///persistent chain metadata storage
    chain_meta_storage: ChainMetaStorage,

    /// Current head information (required, at least genesis should be here)
    current_head: Head,

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
        chain_id: Arc<ChainId>,
    ) -> Self {
        HeadState {
            chain_meta_storage: ChainMetaStorage::new(persistent_storage),
            current_head,
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
    ) -> Result<Option<(Head, HeadResult)>, StateError> {
        // check if we can update head
        {
            let mempool_state = current_mempool_state.read()?;
            let current_context_fitness = {
                // get fitness from mempool, if not, than use current_head.fitness
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
            // release lock asap
            drop(mempool_state);
        }

        // we need to check, if previous head is predecessor of new_head (for later use)
        let head_result = if self
            .current_head
            .block_hash()
            .eq(potential_new_head.header.predecessor())
        {
            HeadResult::HeadIncrement
        } else {
            // if previous head is not predecesor of new head, means it could be new branch
            HeadResult::BranchSwitch
        };

        // this will be new head
        let head = Head::new(
            potential_new_head.hash.clone(),
            potential_new_head.header.level(),
            potential_new_head.header.fitness().clone(),
        );

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
