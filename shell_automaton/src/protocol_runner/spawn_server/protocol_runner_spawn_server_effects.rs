// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::service::ProtocolRunnerService;
use crate::{Action, ActionWithMeta, Service, Store};

use super::ProtocolRunnerSpawnServerPendingAction;

pub fn protocol_runner_spawn_server_effects<S>(store: &mut Store<S>, action: &ActionWithMeta)
where
    S: Service,
{
    match &action.action {
        Action::ProtocolRunnerSpawnServerInit(_) => {
            store.service.protocol_runner().spawn_server();
            store.dispatch(ProtocolRunnerSpawnServerPendingAction {});
        }
        _ => {}
    }
}
