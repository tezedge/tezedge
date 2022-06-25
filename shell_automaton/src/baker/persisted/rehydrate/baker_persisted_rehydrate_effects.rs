// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::baker::block_baker::{
    BakerBlockBakerRightsGetInitAction, BakerBlockBakerStatePersistSuccessAction,
};
use crate::baker::block_endorser::{
    BakerBlockEndorserRightsGetInitAction, BakerBlockEndorserStatePersistSuccessAction,
};
use crate::service::BakerService;
use crate::{Action, ActionWithMeta, Service, Store};

use super::{
    BakerPersistedRehydrateInitAction, BakerPersistedRehydratePendingAction,
    BakerPersistedRehydratedAction,
};

pub fn baker_persisted_rehydrate_effects<S>(store: &mut Store<S>, action: &ActionWithMeta)
where
    S: Service,
{
    match &action.action {
        Action::CurrentHeadRehydrated(_) => {
            let bakers = store.state().baker_keys_iter().cloned().collect::<Vec<_>>();

            for baker in bakers {
                store.dispatch(BakerPersistedRehydrateInitAction { baker });
            }
        }
        Action::BakerPersistedRehydrateInit(content) => {
            let baker = content.baker.clone();
            let chain_id = &store.state.get().config.chain_id;
            let req_id = store
                .service
                .baker()
                .state_rehydrate(baker.clone(), chain_id);

            store.dispatch(BakerPersistedRehydratePendingAction { baker, req_id });
        }
        Action::BakerPersistedRehydrateSuccess(content) => {
            let baker = content.baker.clone();
            store.dispatch(BakerPersistedRehydratedAction { baker });
        }
        Action::BakerPersistedRehydrated(content) => {
            store.dispatch(BakerBlockBakerRightsGetInitAction {});
            store.dispatch(BakerBlockEndorserRightsGetInitAction {});
            store.dispatch(BakerBlockBakerStatePersistSuccessAction {
                baker: content.baker.clone(),
            });
            store.dispatch(BakerBlockEndorserStatePersistSuccessAction {
                baker: content.baker.clone(),
            });
        }
        _ => {}
    }
}
