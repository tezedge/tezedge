// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::{Action, ActionWithId, Service, State, Store};

use crate::service::ProtocolService;

pub fn protocol_effects<S>(store: &mut Store<State, S, Action>, action: &ActionWithId<Action>)
where
    S: Service,
{
    match &action.action {
        Action::WakeupEvent(_) => {
            if let Ok(action) = store.service.protocol().try_recv() {
                store.dispatch(action.into());
            }
        },
        Action::ProtocolEvent(event) => {
            let _ = event;
        },
        _ => (),
    }
}
