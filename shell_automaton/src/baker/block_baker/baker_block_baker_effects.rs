// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::rights::rights_actions::RightsGetAction;
use crate::rights::RightsKey;
use crate::{Action, ActionWithMeta, Service, Store};

use super::{
    BakerBlockBakerRightsGetCurrentLevelSuccessAction, BakerBlockBakerRightsGetInitAction,
    BakerBlockBakerRightsGetNextLevelSuccessAction, BakerBlockBakerRightsGetPendingAction,
    BakerBlockBakerRightsGetSuccessAction, BakerBlockBakerRightsNoRightsAction,
    BakerBlockBakerTimeoutPendingAction,
};

pub fn baker_block_baker_effects<S>(store: &mut Store<S>, action: &ActionWithMeta)
where
    S: Service,
{
    match &action.action {
        Action::CurrentHeadRehydrated(_) | Action::CurrentHeadUpdate(_) => {
            store.dispatch(BakerBlockBakerRightsGetInitAction {});
        }
        Action::BakerBlockBakerRightsGetInit(_) => {
            let (head_level, head_hash) = match store.state().current_head.get() {
                Some(v) => (v.header.level(), v.hash.clone()),
                None => return,
            };

            let bakers = store.state().baker_keys_iter().cloned().collect::<Vec<_>>();
            for baker in bakers {
                store.dispatch(BakerBlockBakerRightsGetPendingAction { baker });
            }

            // TODO(zura): use baking rights instead.
            store.dispatch(RightsGetAction {
                key: RightsKey::endorsing(head_hash, Some(head_level + 1)),
            });
        }
        Action::RightsEndorsingReady(content) => {
            let head = match store.state().current_head.get() {
                Some(v) => v,
                None => return,
            };
            let is_level_current = content
                .key
                .level()
                .map_or(true, |level| level == head.header.level());
            let is_level_next = content
                .key
                .level()
                .map_or(false, |level| level == head.header.level() + 1);
            if content.key.block() != &head.hash || (!is_level_current && !is_level_next) {
                return;
            }
            let rights_level = if is_level_current {
                head.header.level()
            } else if is_level_next {
                head.header.level() + 1
            } else {
                return;
            };

            let rights = store.state().rights.cache.endorsing.get(&rights_level);
            let rights = match rights {
                Some((_, rights)) => rights,
                None => return,
            };
            let bakers_slots = store
                .state()
                .baker_keys_iter()
                .cloned()
                .map(|baker| {
                    let baker_key = baker.clone();
                    rights
                        .delegates
                        .get(&baker)
                        .map(|(first_slot, _)| (baker, vec![*first_slot]))
                        .unwrap_or((baker_key, vec![]))
                })
                .collect::<Vec<_>>();
            for (baker, slots) in bakers_slots {
                if is_level_current {
                    store.dispatch(BakerBlockBakerRightsGetCurrentLevelSuccessAction {
                        baker,
                        slots,
                    });
                } else {
                    store.dispatch(BakerBlockBakerRightsGetNextLevelSuccessAction { baker, slots });
                }
            }
        }
        Action::BakerBlockBakerRightsGetCurrentLevelSuccess(content) => {
            store.dispatch(BakerBlockBakerRightsNoRightsAction {
                baker: content.baker.clone(),
            });
            store.dispatch(BakerBlockBakerRightsGetSuccessAction {
                baker: content.baker.clone(),
            });
        }
        Action::BakerBlockBakerRightsGetNextLevelSuccess(content) => {
            store.dispatch(BakerBlockBakerRightsNoRightsAction {
                baker: content.baker.clone(),
            });
            store.dispatch(BakerBlockBakerRightsGetSuccessAction {
                baker: content.baker.clone(),
            });
        }
        Action::BakerBlockBakerRightsGetSuccess(content) => {
            store.dispatch(BakerBlockBakerTimeoutPendingAction {
                baker: content.baker.clone(),
            });
        }
        _ => {}
    }
}
