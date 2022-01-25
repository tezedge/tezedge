// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::storage::blocks::genesis::check_applied::StorageBlocksGenesisCheckAppliedInitAction;
use crate::{Action, ActionWithMeta, Service, Store};

use super::context::ProtocolRunnerInitContextAction;
use super::context_ipc_server::ProtocolRunnerInitContextIpcServerAction;
use super::runtime::ProtocolRunnerInitRuntimeAction;
use super::{
    ProtocolRunnerInitCheckGenesisAppliedAction,
    ProtocolRunnerInitCheckGenesisAppliedSuccessAction, ProtocolRunnerInitSuccessAction,
};

pub fn protocol_runner_init_effects<S>(store: &mut Store<S>, action: &ActionWithMeta)
where
    S: Service,
{
    match &action.action {
        Action::ProtocolRunnerInit(_) => {
            store.dispatch(ProtocolRunnerInitRuntimeAction {});
        }
        Action::ProtocolRunnerInitRuntimeSuccess(_) => {
            store.dispatch(ProtocolRunnerInitCheckGenesisAppliedAction {});
        }
        Action::ProtocolRunnerInitCheckGenesisApplied(_) => {
            store.dispatch(StorageBlocksGenesisCheckAppliedInitAction {});
        }
        Action::StorageBlocksGenesisCheckAppliedSuccess(content) => {
            store.dispatch(ProtocolRunnerInitCheckGenesisAppliedSuccessAction {
                is_applied: content.is_applied,
            });
        }
        Action::ProtocolRunnerInitCheckGenesisAppliedSuccess(_) => {
            store.dispatch(ProtocolRunnerInitContextAction {});
        }
        Action::ProtocolRunnerInitContextSuccess(_) => {
            store.dispatch(ProtocolRunnerInitContextIpcServerAction {});
        }
        Action::ProtocolRunnerInitContextIpcServerSuccess(_) => {
            store.dispatch(ProtocolRunnerInitSuccessAction {});
        }
        _ => {}
    }
}
