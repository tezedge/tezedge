// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::Action;

use super::{rights_cycle_delegates_actions::*, CycleDelegatesQuery, CycleDelegatesQueryState};

pub fn rights_cycle_delegates_reducer(state: &mut crate::State, action: &crate::ActionWithMeta) {
    let delegates_state = &mut state.rights.cycle_delegates;
    match &action.action {
        Action::RightsCycleDelegatesGet(RightsCycleDelegatesGetAction { cycle, .. }) => {
            delegates_state.insert(
                *cycle,
                CycleDelegatesQuery {
                    state: CycleDelegatesQueryState::Init,
                },
            );
        }
        Action::RightsCycleDelegatesRequested(RightsCycleDelegatesRequestedAction {
            cycle,
            token,
        }) => {
            if let Some(req) = delegates_state.get_mut(cycle) {
                req.state = CycleDelegatesQueryState::ContextRequested(*token);
            }
        }
        Action::RightsCycleDelegatesSuccess(RightsCycleDelegatesSuccessAction {
            cycle,
            delegates,
        }) => {
            if let Some(req) = delegates_state.get_mut(cycle) {
                req.state = CycleDelegatesQueryState::Success(delegates.clone());
            }
        }
        Action::RightsCycleDelegatesError(RightsCycleDelegatesErrorAction { cycle, error }) => {
            if let Some(req) = delegates_state.get_mut(cycle) {
                req.state = CycleDelegatesQueryState::Error(error.clone());
            }
        }
        _ => (),
    }
}
