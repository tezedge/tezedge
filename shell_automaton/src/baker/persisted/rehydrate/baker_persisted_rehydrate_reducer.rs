// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::baker::block_baker::BakerBlockBakerState;
use crate::baker::block_endorser::BakerBlockEndorserState;
use crate::{Action, ActionWithMeta, State};

use super::BakerPersistedRehydrateState;

pub fn baker_persisted_rehydrate_reducer(state: &mut State, action: &ActionWithMeta) {
    match &action.action {
        Action::BakerPersistedRehydratePending(content) => {
            state.bakers.get_mut(&content.baker).and_then(|baker| {
                baker.persisted.rehydrate = BakerPersistedRehydrateState::Pending {
                    time: action.time_as_nanos(),
                    req_id: content.req_id,
                };

                Some(())
            });
        }
        Action::BakerPersistedRehydrateSuccess(content) => {
            state.bakers.get_mut(&content.baker).and_then(|baker| {
                baker.persisted.rehydrate = BakerPersistedRehydrateState::Success {
                    time: action.time_as_nanos(),
                    result: content.result.clone(),
                };

                Some(())
            });
        }
        Action::BakerPersistedRehydrated(content) => {
            state.bakers.get_mut(&content.baker).and_then(|baker| {
                let rehydrated_state = match &baker.persisted.rehydrate {
                    BakerPersistedRehydrateState::Success { result, .. } => result.clone(),
                    _ => return None,
                };
                let last_persisted_counter = rehydrated_state.counter();
                baker.persisted.rehydrate = BakerPersistedRehydrateState::Rehydrated {
                    time: action.time_as_nanos(),
                    current_state: rehydrated_state.clone(),
                    rehydrated_state,
                    last_persisted_counter,
                };

                let rehydrated_state = baker.persisted.current_state()?;

                if let Some(data) = rehydrated_state.last_baked_block {
                    baker.block_baker = BakerBlockBakerState::StatePersistPending {
                        time: action.time_as_nanos(),
                        state_counter: last_persisted_counter,
                        header: data.header.clone(),
                        operations: data.operations.clone(),
                    };
                }
                if let Some(data) = rehydrated_state.last_endorsement {
                    baker.block_endorser = BakerBlockEndorserState::StatePersistPending {
                        time: action.time_as_nanos(),
                        state_counter: last_persisted_counter,
                        first_slot: data.operation.operation().slot,
                        operation: data.operation.clone(),
                        signature: data.signature.clone(),
                    };
                }

                baker.seed_nonces = rehydrated_state.seed_nonces.clone();

                Some(())
            });
        }
        _ => {}
    }
}
