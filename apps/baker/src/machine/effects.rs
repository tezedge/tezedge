// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use redux_rs::{Store, ActionWithMeta, TimeService};

use crate::services::BakerService;

use super::{BakerState, BakerAction};

pub fn baker_effects<S, Srv, A>(store: &mut Store<S, Srv, A>, action: &ActionWithMeta<A>)
where
    S: AsRef<Option<BakerState>>,
    Srv: TimeService + BakerService,
    A: AsRef<Option<BakerAction>>,
{
    if let Some(baker_state) = store.state.get().as_ref() {
        for action_inner in &baker_state.as_ref().actions {
            store.service.execute(action_inner);
        }
    }

    match action.action.as_ref() {
        None => (),
        // don't handle events here
        Some(BakerAction::RpcError(_) | BakerAction::IdleEvent(_) | BakerAction::ProposalEvent(_) | BakerAction::SlotsEvent(_) | BakerAction::OperationsForBlockEvent(_) | BakerAction::LiveBlocksEvent(_) | BakerAction::OperationsEvent(_) | BakerAction::TickEvent(_)) => (),
    }
}
