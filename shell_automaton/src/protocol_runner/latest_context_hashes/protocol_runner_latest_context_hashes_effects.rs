// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::protocol_runner::ProtocolRunnerReadyAction;
use crate::service::ProtocolRunnerService;
use crate::{Action, ActionWithMeta, Service, Store};

use super::{ProtocolRunnerLatestContextHashesPendingAction, DEFAULT_NUMBER_OF_CONTEXT_HASHES};

pub fn protocol_runner_latest_context_hashes_effects<S>(
    store: &mut Store<S>,
    action: &ActionWithMeta,
) where
    S: Service,
{
    match &action.action {
        Action::ProtocolRunnerLatestContextHashesInit(_) => {
            let count = DEFAULT_NUMBER_OF_CONTEXT_HASHES;
            let token = store
                .service
                .protocol_runner()
                .get_latest_context_hashes(count);
            store.dispatch(ProtocolRunnerLatestContextHashesPendingAction { token });
        }
        Action::ProtocolRunnerLatestContextHashesSuccess(content) => {
            slog::info!(&store.state().log, "Found context's latest commits";
                         "context_hashes" => format!("{:?}", content.latest_context_hashes));
            store.dispatch(ProtocolRunnerReadyAction {});
        }
        Action::ProtocolRunnerLatestContextHashesError(content) => {
            slog::error!(&store.state().log, "Failed to get context's latest commits";
                         "error" => format!("{:?}", content.error));
            store.dispatch(ProtocolRunnerReadyAction {});
        }
        _ => {}
    }
}
