// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! This module covers helper functionality with managing current head attirbute for chain managers

use std::fmt;
use std::sync::{Arc, RwLock};

use crypto::hash::{BlockHash, ChainId};
use storage::chain_meta_storage::ChainMetaStorageReader;
use storage::persistent::PersistentStorage;
use storage::{BlockHeaderWithHash, ChainMetaStorage};
use tezos_messages::Head;

use crate::mempool::CurrentMempoolStateStorageRef;
use crate::state::StateError;
use crate::validation;

/// In-memory synchronized struct for sharing current head between threads/actors
pub type CurrentHeadRef = Arc<RwLock<Option<Head>>>;

/// Inits empty current head state
pub fn init_current_head_state() -> CurrentHeadRef {
    Arc::new(RwLock::new(None))
}

pub enum HeadResult {
    BranchSwitch,
    HeadIncrement,
    GenesisInitialized,
}

impl fmt::Display for HeadResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            HeadResult::BranchSwitch => write!(f, "BranchSwitch"),
            HeadResult::HeadIncrement => write!(f, "HeadIncrement"),
            HeadResult::GenesisInitialized => write!(f, "GenesisInitialized"),
        }
    }
}

pub struct HeadState {
    ///persistent chain metadata storage
    chain_meta_storage: ChainMetaStorage,

    /// Current head information
    current_head_state: CurrentHeadRef,
    /// Holds ref to global current shared mempool state
    current_mempool_state: CurrentMempoolStateStorageRef,

    chain_id: Arc<ChainId>,
    chain_genesis_block_hash: Arc<BlockHash>,
}

impl HeadState {
    pub fn new(
        persistent_storage: &PersistentStorage,
        current_head_state: CurrentHeadRef,
        current_mempool_state: CurrentMempoolStateStorageRef,
        chain_id: Arc<ChainId>,
        chain_genesis_block_hash: Arc<BlockHash>,
    ) -> Self {
        HeadState {
            chain_meta_storage: ChainMetaStorage::new(persistent_storage),
            current_head_state,
            current_mempool_state,
            chain_id,
            chain_genesis_block_hash,
        }
    }

    /// Resolve if new applied block can be set as new current head.
    /// Original algorithm is in [chain_validator][on_request], where just fitness is checked.
    /// Returns:
    /// - None, if head was not updated, means was ignored
    /// - Some(new_head, head_result)
    pub fn try_update_new_current_head(
        &self,
        potential_new_head: &BlockHeaderWithHash,
    ) -> Result<Option<(Head, HeadResult)>, StateError> {
        // check if we can update head
        if let Some(current_head) = self.current_head_state.read()?.as_ref() {
            // get fitness from mempool, if not, than use current_head.fitness
            let mempool_state = self.current_mempool_state.read()?;
            let current_context_fitness = {
                if let Some(Some(fitness)) = mempool_state
                    .prevalidator()
                    .map(|p| p.context_fitness.as_ref())
                {
                    fitness
                } else {
                    current_head.fitness()
                }
            };
            // need to check against current_head, if not accepted, just ignore potential head
            if !validation::can_update_current_head(
                potential_new_head,
                &current_head,
                &current_context_fitness,
            ) {
                // just ignore
                return Ok(None);
            }
        }

        // we need to check, if previous head is predecessor of new_head (for later use)
        let head_result = match self.current_head_state.read()?.as_ref() {
            Some(previos_head) => {
                if previos_head
                    .block_hash()
                    .eq(potential_new_head.header.predecessor())
                {
                    HeadResult::HeadIncrement
                } else {
                    // if previous head is not predecesor of new head, means it could be new branch
                    HeadResult::BranchSwitch
                }
            }
            None => {
                // we check, if new head is genesis
                if self
                    .chain_genesis_block_hash
                    .as_ref()
                    .eq(&potential_new_head.hash)
                {
                    HeadResult::GenesisInitialized
                } else {
                    HeadResult::HeadIncrement
                }
            }
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
        let mut current_head_state = self.current_head_state.write()?;
        *current_head_state = Some(head.clone());

        Ok(Some((head, head_result)))
    }

    /// Tries to load last known current head from database
    pub(crate) fn load_current_head_state(&self) -> Result<Option<Head>, StateError> {
        match self.chain_meta_storage.get_current_head(&self.chain_id)? {
            Some(head) => {
                let mut current_head_state = self.current_head_state.write()?;
                *current_head_state = Some(head.clone());
                Ok(Some(head))
            }
            None => Ok(None),
        }
    }
}
