// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use crypto::hash::BlockHash;
use tezos_messages::base::signature_public_key::SignaturePublicKey;
use tezos_messages::p2p::encoding::block_header::Level;

use crate::baker::block_baker::BakerBlockBakerState;
use crate::mempool::OperationKind;
use crate::{EnablingCondition, State};

use super::{
    BakerSeedNonceState, SeedNonce, SeedNonceHash, SeedNonceRevelationOperationWithForgedBytes,
};

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerSeedNonceGeneratedAction {
    pub baker: SignaturePublicKey,
    pub level: Level,
    pub nonce: SeedNonce,
    pub nonce_hash: SeedNonceHash,
}

impl EnablingCondition<State> for BakerSeedNonceGeneratedAction {
    fn is_enabled(&self, state: &State) -> bool {
        let blocks_per_commitment = match state.current_head.constants() {
            Some(v) => v.blocks_per_commitment,
            None => return false,
        };
        state.bakers.get(&self.baker).map_or(false, |baker| {
            !baker.seed_nonces.contains_key(&self.level) && self.level % blocks_per_commitment == 0
        })
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerSeedNonceCommittedAction {
    pub baker: SignaturePublicKey,
    pub level: Level,
    pub block_hash: BlockHash,
}

impl EnablingCondition<State> for BakerSeedNonceCommittedAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .bakers
            .get(&self.baker)
            .and_then(|baker| {
                let nonce_state = baker.seed_nonces.get(&self.level)?;
                match nonce_state {
                    BakerSeedNonceState::Generated { .. } => {}
                    _ => {
                        return None;
                    }
                }
                Some(match &baker.block_baker {
                    BakerBlockBakerState::InjectPending { block, .. } => {
                        block.header.level() == self.level && block.hash == self.block_hash
                    }
                    _ => false,
                })
            })
            .unwrap_or(false)
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerSeedNonceCycleNextWaitAction {
    pub baker: SignaturePublicKey,
    pub level: Level,
}

impl EnablingCondition<State> for BakerSeedNonceCycleNextWaitAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .bakers
            .get(&self.baker)
            .and_then(|baker| {
                let nonce_state = baker.seed_nonces.get(&self.level)?;
                let cycle = match nonce_state {
                    BakerSeedNonceState::Committed { cycle, .. } => *cycle,
                    _ => {
                        return None;
                    }
                };
                state
                    .current_head
                    .cycle_info()
                    .map(|cur_cycle| cur_cycle.cycle <= cycle && cur_cycle.position >= 2)
            })
            .unwrap_or(false)
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerSeedNonceRevealInitAction {
    pub baker: SignaturePublicKey,
    pub level: Level,
}

impl BakerSeedNonceRevealInitAction {
    fn should_reveal(state: &State, nonce_state: &BakerSeedNonceState) -> bool {
        match nonce_state {
            BakerSeedNonceState::CycleNextWait { cycle, .. } => state
                .current_head
                .cycle_info()
                .map_or(false, |cur_cycle| cur_cycle.cycle == *cycle),
            _ => false,
        }
    }
}

impl EnablingCondition<State> for BakerSeedNonceRevealInitAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .bakers
            .get(&self.baker)
            .and_then(|baker| {
                let nonce_state = baker.seed_nonces.get(&self.level)?;
                Some(Self::should_reveal(state, nonce_state))
            })
            .unwrap_or(false)
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerSeedNonceRevealPendingAction {
    pub baker: SignaturePublicKey,
    pub level: Level,
    pub operation: SeedNonceRevelationOperationWithForgedBytes,
}

impl EnablingCondition<State> for BakerSeedNonceRevealPendingAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .bakers
            .get(&self.baker)
            .and_then(|baker| {
                let nonce_state = baker.seed_nonces.get(&self.level)?;
                Some(BakerSeedNonceRevealInitAction::should_reveal(
                    state,
                    nonce_state,
                ))
            })
            .unwrap_or(false)
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerSeedNonceRevealMempoolInjectAction {
    pub baker: SignaturePublicKey,
    pub level: Level,
}

impl EnablingCondition<State> for BakerSeedNonceRevealMempoolInjectAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .bakers
            .get(&self.baker)
            .and_then(|baker| baker.seed_nonces.get(&self.level))
            .map_or(false, |s| match s {
                BakerSeedNonceState::RevealPending { injected_in, .. } => {
                    let branch = match state.current_head.pred_hash() {
                        Some(v) => v,
                        None => return false,
                    };
                    injected_in.len() <= 32
                        && !injected_in.contains(branch)
                        && state.is_bootstrapped()
                }
                _ => false,
            })
    }
}

/// Seed nonce revelation operation was included in the block.
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerSeedNonceRevealIncludedAction {
    pub baker: SignaturePublicKey,
    pub level: Level,
}

impl EnablingCondition<State> for BakerSeedNonceRevealIncludedAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .bakers
            .get(&self.baker)
            .and_then(|baker| baker.seed_nonces.get(&self.level))
            .and_then(|s| match s {
                BakerSeedNonceState::RevealPending {
                    included_in,
                    operation,
                    ..
                } => {
                    let head_hash = state.current_head.hash()?;
                    let head_operations = state.current_head.operations()?.get(2)?;

                    if included_in.contains(head_hash) {
                        return None;
                    }
                    Some(
                        head_operations
                            .iter()
                            .filter(|op| {
                                matches!(
                                    OperationKind::from_operation_content_raw(op.data().as_ref()),
                                    OperationKind::SeedNonceRevelation
                                )
                            })
                            .any(|op| {
                                let op_data_bytes: &[u8] = op.data().as_ref();
                                operation.forged() == op_data_bytes
                            }),
                    )
                }
                _ => None,
            })
            .unwrap_or(false)
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerSeedNonceRevealSuccessAction {
    pub baker: SignaturePublicKey,
    pub level: Level,
}

impl EnablingCondition<State> for BakerSeedNonceRevealSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .bakers
            .get(&self.baker)
            .and_then(|baker| {
                let nonce_state = baker.seed_nonces.get(&self.level)?;
                let cemented_block_hash = state.current_head.cemented_block_hash()?;
                Some(match nonce_state {
                    BakerSeedNonceState::RevealPending { included_in, .. } => {
                        included_in.contains(cemented_block_hash)
                    }
                    _ => false,
                })
            })
            .unwrap_or(false)
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BakerSeedNonceFinishAction {
    pub baker: SignaturePublicKey,
    pub level: Level,
}

impl EnablingCondition<State> for BakerSeedNonceFinishAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .bakers
            .get(&self.baker)
            .and_then(|baker| baker.seed_nonces.get(&self.level))
            .map_or(false, |s| match s {
                BakerSeedNonceState::RevealPending { injected_in, .. } => injected_in.len() > 32,
                BakerSeedNonceState::RevealSuccess { .. } => true,
                _ => false,
            })
    }
}
