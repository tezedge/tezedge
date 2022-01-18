// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::protocol_runner::ProtocolRunnerShutdownInitAction;
use crate::{Action, ActionWithMeta, Service, Store};

use super::{ShutdownPendingAction, ShutdownSuccessAction};

pub fn shutdown_effects<S>(store: &mut Store<S>, action: &ActionWithMeta)
where
    S: Service,
{
    match &action.action {
        Action::ShutdownInit(_) => {
            store.dispatch(ProtocolRunnerShutdownInitAction {});
            store.dispatch(ShutdownPendingAction {});
        }
        Action::ShutdownPending(_) | Action::ProtocolRunnerShutdownSuccess(_) => {
            // Enabling condition for `ShutdownSuccessAction` will be checked
            // and if indeed shutdown was successful, this action will be dispatched.
            store.dispatch(ShutdownSuccessAction {});
        }
        _ => {}
    }
}
