// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use redux_rs::{Store, ActionWithMeta, TimeService};

use super::{BakerState, BakerAction};

pub fn baker_effects<S, Srv, A>(store: &mut Store<S, Srv, A>, action: &ActionWithMeta<A>)
where
    S: AsRef<Option<BakerState>>,
    Srv: TimeService,
    A: AsRef<Option<BakerAction>>,
{
    if let Some(baker_state) = store.state().as_ref() {
        for action_inner in &baker_state.as_ref().actions {
            // TODO: dispatch
            let _ = action_inner;
            // store.dispatch(action);
        }
    }

    match action.action.as_ref() {
        None => (),
        // don't handle events here
        Some(BakerAction::IdleEvent(_) | BakerAction::ProposalEvent(_)) => (),
    }
}
