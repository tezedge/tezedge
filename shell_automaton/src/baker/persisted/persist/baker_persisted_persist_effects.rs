// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::baker::block_baker::BakerBlockBakerStatePersistSuccessAction;
use crate::baker::block_endorser::BakerBlockEndorserStatePersistSuccessAction;
use crate::service::BakerService;
use crate::{Action, ActionWithMeta, Service, Store};

use super::{BakerPersistedPersistInitAction, BakerPersistedPersistPendingAction};

pub fn baker_persisted_persist_effects<S>(store: &mut Store<S>, action: &ActionWithMeta)
where
    S: Service,
{
    match &action.action {
        Action::BakerPersistedPersistInit(content) => {
            let baker = content.baker.clone();
            let baker_state = store.state().bakers.get(&baker);
            let new_state = match baker_state.and_then(|b| b.persisted.current_state()) {
                Some(v) => v.cloned(),
                None => return,
            };
            let counter = new_state.counter();
            let chain_id = &store.state.get().config.chain_id;
            let req_id = store
                .service
                .baker()
                .state_persist(baker.clone(), chain_id, new_state);
            store.dispatch(BakerPersistedPersistPendingAction {
                baker,
                req_id,
                counter,
            });
        }
        Action::BakerPersistedPersistSuccess(content) => {
            let baker = content.baker.clone();
            store.dispatch(BakerBlockBakerStatePersistSuccessAction {
                baker: baker.clone(),
            });
            store.dispatch(BakerBlockEndorserStatePersistSuccessAction {
                baker: baker.clone(),
            });
            store.dispatch(BakerPersistedPersistInitAction { baker });
        }
        _ => {}
    }
}
