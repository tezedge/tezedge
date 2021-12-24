// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::{Action, ActionWithMeta, Service, Store};

use crate::service::ProtocolService;

pub fn protocol_effects<S>(store: &mut Store<S>, action: &ActionWithMeta)
where
    S: Service,
{
    match &action.action {
        Action::WakeupEvent(_) => {
            while let Ok(action) = store.service.protocol().try_recv() {
                store.dispatch(action);
            }
        }
        _ => (),
    }
}
