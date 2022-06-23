// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::{Action, ActionWithMeta, State};

use super::BakerPersistedPersistState;

pub fn baker_persisted_persist_reducer(state: &mut State, action: &ActionWithMeta) {
    match &action.action {
        Action::BakerPersistedPersistPending(content) => {
            state.bakers.get_mut(&content.baker).and_then(|baker| {
                baker.persisted.persist = BakerPersistedPersistState::Pending {
                    time: action.time_as_nanos(),
                    req_id: content.req_id,
                    counter: content.counter,
                };

                Some(())
            });
        }
        Action::BakerPersistedPersistSuccess(content) => {
            state.bakers.get_mut(&content.baker).and_then(|baker| {
                let counter = baker.persisted.persist.counter();
                baker.persisted.set_last_persisted_counter(counter);
                baker.persisted.persist = BakerPersistedPersistState::Success {
                    time: action.time_as_nanos(),
                    counter,
                };

                Some(())
            });
        }
        _ => {}
    }
}
