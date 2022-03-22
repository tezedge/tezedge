// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::{Action, ActionWithMeta, Service, Store};

use crate::service::PrevalidatorService;

pub fn protocol_effects<S>(store: &mut Store<S>, action: &ActionWithMeta)
where
    S: Service,
{
    if let Action::WakeupEvent(_) = &action.action {
        while let Ok(action) = store.service.prevalidator().try_recv() {
            store.dispatch(action);
        }
    }
}
