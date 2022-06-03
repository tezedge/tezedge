// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::{Action, ActionWithMeta, State};

use super::BakerSeedNonceState;

pub fn baker_seed_nonce_reducer(state: &mut State, action: &ActionWithMeta) {
    match &action.action {
        Action::BakerSeedNonceGenerated(content) => {
            state.bakers.get_mut(&content.baker).and_then(|baker| {
                let constants = state.current_head.constants()?;
                let head_level = state.current_head.level()?;
                let head_cycle = state.current_head.cycle_info()?;
                let cycle = if content.level > head_level
                    && head_cycle.position + 1 == constants.blocks_per_cycle
                {
                    head_cycle.cycle + 1
                } else {
                    head_cycle.cycle
                };
                baker.seed_nonces.insert(
                    content.level,
                    BakerSeedNonceState::Generated {
                        time: action.time_as_nanos(),
                        cycle,
                        nonce: content.nonce.clone(),
                        nonce_hash: content.nonce_hash.clone(),
                    },
                );

                Some(())
            });
        }
        Action::BakerSeedNonceCommitted(content) => {
            state.bakers.get_mut(&content.baker).and_then(|baker| {
                let nonce_state = baker.seed_nonces.get_mut(&content.level)?;
                match nonce_state {
                    BakerSeedNonceState::Generated {
                        cycle,
                        nonce,
                        nonce_hash,
                        ..
                    } => {
                        *nonce_state = BakerSeedNonceState::Committed {
                            time: action.time_as_nanos(),
                            cycle: *cycle,
                            nonce: nonce.clone(),
                            nonce_hash: nonce_hash.clone(),
                        };
                        Some(())
                    }
                    _ => None,
                }
            });
        }
        Action::BakerSeedNonceCycleNextWait(content) => {
            state.bakers.get_mut(&content.baker).and_then(|baker| {
                let nonce_state = baker.seed_nonces.get_mut(&content.level)?;
                match nonce_state {
                    BakerSeedNonceState::Committed {
                        cycle,
                        nonce,
                        nonce_hash,
                        ..
                    } => {
                        *nonce_state = BakerSeedNonceState::CycleNextWait {
                            time: action.time_as_nanos(),
                            cycle: *cycle + 1,
                            nonce: nonce.clone(),
                            nonce_hash: nonce_hash.clone(),
                        };
                        Some(())
                    }
                    _ => None,
                }
            });
        }
        Action::BakerSeedNonceRevealPending(content) => {
            state.bakers.get_mut(&content.baker).and_then(|baker| {
                let nonce_state = baker.seed_nonces.get_mut(&content.level)?;
                match nonce_state {
                    BakerSeedNonceState::CycleNextWait {
                        cycle, nonce_hash, ..
                    } => {
                        *nonce_state = BakerSeedNonceState::RevealPending {
                            time: action.time_as_nanos(),
                            cycle: *cycle,
                            injected_in: Default::default(),
                            included_in: Default::default(),
                            operation: content.operation.clone(),
                            nonce_hash: nonce_hash.clone(),
                        };
                        Some(())
                    }
                    _ => None,
                }
            });
        }
        Action::BakerSeedNonceRevealMempoolInject(content) => {
            state.bakers.get_mut(&content.baker).and_then(|baker| {
                let nonce_state = baker.seed_nonces.get_mut(&content.level)?;
                match nonce_state {
                    BakerSeedNonceState::RevealPending { injected_in, .. } => {
                        let head_hash = state.current_head.pred_hash()?.clone();
                        injected_in.insert(head_hash);
                        Some(())
                    }
                    _ => None,
                }
            });
        }
        Action::BakerSeedNonceRevealIncluded(content) => {
            state.bakers.get_mut(&content.baker).and_then(|baker| {
                let nonce_state = baker.seed_nonces.get_mut(&content.level)?;
                match nonce_state {
                    BakerSeedNonceState::RevealPending { included_in, .. } => {
                        let head_hash = state.current_head.hash()?.clone();
                        included_in.insert(head_hash);
                        Some(())
                    }
                    _ => None,
                }
            });
        }
        Action::BakerSeedNonceRevealSuccess(content) => {
            state.bakers.get_mut(&content.baker).and_then(|baker| {
                let nonce_state = baker.seed_nonces.get_mut(&content.level)?;
                match nonce_state {
                    BakerSeedNonceState::RevealPending {
                        cycle,
                        operation,
                        nonce_hash,
                        ..
                    } => {
                        let block_hash = state.current_head.cemented_block_hash()?.clone();
                        *nonce_state = BakerSeedNonceState::RevealSuccess {
                            time: action.time_as_nanos(),
                            cycle: *cycle,
                            block_hash,
                            operation: operation.clone(),
                            nonce_hash: nonce_hash.clone(),
                        };

                        Some(())
                    }
                    _ => None,
                }
            });
        }
        _ => {}
    }
}
