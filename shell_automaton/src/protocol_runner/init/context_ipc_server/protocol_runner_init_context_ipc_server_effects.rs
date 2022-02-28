// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::protocol_runner::init::context_ipc_server::ProtocolRunnerInitContextIpcServerSuccessAction;
use crate::service::ProtocolRunnerService;
use crate::{Action, ActionWithMeta, Service, Store};

use super::ProtocolRunnerInitContextIpcServerPendingAction;

pub fn protocol_runner_init_context_ipc_server_effects<S>(
    store: &mut Store<S>,
    action: &ActionWithMeta,
) where
    S: Service,
{
    match &action.action {
        Action::ProtocolRunnerInitContextIpcServer(_) => {
            let cfg = &store.state.get().config.protocol_runner.storage;

            if cfg.get_ipc_socket_path().is_none() {
                store.dispatch(ProtocolRunnerInitContextIpcServerSuccessAction { token: None });
                return;
            }

            let token = store
                .service
                .protocol_runner()
                .init_context_ipc_server(cfg.clone());
            store.dispatch(ProtocolRunnerInitContextIpcServerPendingAction { token });
        }
        _ => {}
    }
}
