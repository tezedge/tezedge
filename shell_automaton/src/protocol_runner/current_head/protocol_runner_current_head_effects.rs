// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::protocol_runner::ProtocolRunnerReadyAction;
use crate::service::ProtocolRunnerService;
use crate::{Action, ActionWithMeta, Service, Store};

use super::{ProtocolRunnerCurrentHeadPendingAction, DEFAULT_NUMBER_OF_CONTEXT_HASHES};

pub fn protocol_runner_current_head_effects<S>(store: &mut Store<S>, action: &ActionWithMeta)
where
    S: Service,
{
    match &action.action {
        Action::ProtocolRunnerCurrentHeadInit(_) => {
            let count = DEFAULT_NUMBER_OF_CONTEXT_HASHES;
            let token = store
                .service
                .protocol_runner()
                .get_latest_context_hashes(count);
            store.dispatch(ProtocolRunnerCurrentHeadPendingAction { token });
        }
        Action::ProtocolRunnerCurrentHeadSuccess(_) => {
            store.dispatch(ProtocolRunnerReadyAction {});
        }
        _ => {}
    }
}
