// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::protocol_runner::init::context::{
    ProtocolRunnerInitContextErrorAction, ProtocolRunnerInitContextState,
};
use crate::protocol_runner::init::ProtocolRunnerInitState;
use crate::protocol_runner::ProtocolRunnerState;
use crate::service::ProtocolRunnerService;
use crate::{Action, ActionWithMeta, Service, Store};

use super::ProtocolRunnerInitContextPendingAction;

pub fn protocol_runner_init_context_effects<S>(store: &mut Store<S>, action: &ActionWithMeta)
where
    S: Service,
{
    if let Action::ProtocolRunnerInitContext(_) = &action.action {
        let state = store.state.get();
        let apply_genesis = match &state.protocol_runner {
            ProtocolRunnerState::Init(ProtocolRunnerInitState::Context(
                ProtocolRunnerInitContextState::Init { apply_genesis },
            )) => *apply_genesis,
            _ => return,
        };
        let res = store.service.protocol_runner().init_context(
            state.config.protocol_runner.storage.clone(),
            &state.config.protocol_runner.environment,
            apply_genesis,
            state.config.protocol_runner.enable_testchain,
            false,
            state.config.init_storage_data.patch_context.clone(),
            state.config.init_storage_data.context_stats_db_path.clone(),
        );
        match res {
            Ok(token) => store.dispatch(ProtocolRunnerInitContextPendingAction { token }),
            Err(error) => {
                store.dispatch(ProtocolRunnerInitContextErrorAction { token: None, error })
            }
        };
    }
}
