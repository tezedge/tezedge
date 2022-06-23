// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crypto::hash::Signature;
use tezos_messages::p2p::encoding::block_header::{BlockHeader, Level};
use tezos_messages::p2p::encoding::operation::Operation;

use crate::baker::block_endorser::EndorsementWithForgedBytes;
use crate::baker::seed_nonce::BakerSeedNonceState;

use super::persist::BakerPersistedPersistState;
use super::rehydrate::BakerPersistedRehydrateState;

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LastBakedBlock {
    pub header: BlockHeader,
    pub operations: Vec<Vec<Operation>>,
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LastEndorsement {
    pub operation: EndorsementWithForgedBytes,
    pub signature: Signature,
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PersistedState {
    counter: u64,
    last_baked_block: Option<LastBakedBlock>,
    last_endorsement: Option<LastEndorsement>,
    #[serde(default)]
    seed_nonces: BTreeMap<Level, BakerSeedNonceState>,
}

impl PersistedState {
    pub fn counter(&self) -> u64 {
        self.counter
    }
}

impl Default for PersistedState {
    fn default() -> Self {
        Self {
            counter: 0,
            last_baked_block: None,
            last_endorsement: None,
            seed_nonces: Default::default(),
        }
    }
}

#[derive(Debug)]
pub struct PersistedStateRef<'a> {
    pub counter: u64,
    pub last_baked_block: Option<&'a LastBakedBlock>,
    pub last_endorsement: Option<&'a LastEndorsement>,
    pub seed_nonces: &'a BTreeMap<Level, BakerSeedNonceState>,
}

impl<'a> PersistedStateRef<'a> {
    pub fn cloned(&self) -> PersistedState {
        PersistedState {
            counter: self.counter,
            last_baked_block: self.last_baked_block.cloned(),
            last_endorsement: self.last_endorsement.cloned(),
            seed_nonces: self.seed_nonces.clone(),
        }
    }
}

#[derive(Debug)]
pub struct PersistedStateMut<'a> {
    pub counter: u64,
    pub last_baked_block: &'a mut Option<LastBakedBlock>,
    pub last_endorsement: &'a mut Option<LastEndorsement>,
    pub seed_nonces: &'a mut BTreeMap<Level, BakerSeedNonceState>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerPersistedState {
    pub rehydrate: BakerPersistedRehydrateState,
    pub persist: BakerPersistedPersistState,
}

impl BakerPersistedState {
    pub fn new() -> Self {
        Self {
            rehydrate: BakerPersistedRehydrateState::Idle { time: 0 },
            persist: BakerPersistedPersistState::Idle { time: 0 },
        }
    }

    pub fn is_rehydrated(&self) -> bool {
        self.rehydrate.is_rehydrated()
    }

    pub fn current_state<'a>(&'a self) -> Option<PersistedStateRef<'a>> {
        match &self.rehydrate {
            BakerPersistedRehydrateState::Rehydrated { current_state, .. } => {
                Some(PersistedStateRef {
                    counter: current_state.counter,
                    last_baked_block: current_state.last_baked_block.as_ref(),
                    last_endorsement: current_state.last_endorsement.as_ref(),
                    seed_nonces: &current_state.seed_nonces,
                })
            }
            _ => None,
        }
    }

    pub fn update<F>(&mut self, f: F) -> Option<u64>
    where
        F: for<'a> FnOnce(PersistedStateMut<'a>),
    {
        match &mut self.rehydrate {
            BakerPersistedRehydrateState::Rehydrated { current_state, .. } => {
                current_state.counter += 1;
                f(PersistedStateMut {
                    counter: current_state.counter,
                    last_baked_block: &mut current_state.last_baked_block,
                    last_endorsement: &mut current_state.last_endorsement,
                    seed_nonces: &mut current_state.seed_nonces,
                });
                Some(current_state.counter)
            }
            _ => None,
        }
    }

    pub fn last_persisted_counter(&self) -> u64 {
        match &self.rehydrate {
            BakerPersistedRehydrateState::Rehydrated {
                last_persisted_counter,
                ..
            } => *last_persisted_counter,
            _ => 0,
        }
    }

    pub(super) fn set_last_persisted_counter(&mut self, counter: u64) {
        match &mut self.rehydrate {
            BakerPersistedRehydrateState::Rehydrated {
                last_persisted_counter,
                ..
            } => {
                *last_persisted_counter = counter;
            }
            _ => {}
        }
    }
}
