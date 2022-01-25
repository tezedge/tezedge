// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::service::ProtocolRunnerService;
use crate::{Action, ActionWithMeta, Service, Store};

use super::ProtocolRunnerInitRuntimePendingAction;

pub fn protocol_runner_init_runtime_effects<S>(store: &mut Store<S>, action: &ActionWithMeta)
where
    S: Service,
{
    match &action.action {
        Action::ProtocolRunnerInitRuntime(_) => {
            let config = store
                .state()
                .config
                .protocol_runner
                .runtime_configuration
                .clone();
            let token = store.service.protocol_runner().init_runtime(config);
            store.dispatch(ProtocolRunnerInitRuntimePendingAction { token });
        }
        _ => {}
    }
}
